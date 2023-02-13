use crate::codec::{
    check_property, get_bin, encode_bin_property, put_bin,
    encode_u16_property, encode_u32_property, encode_u8_property, encode_utf8_property, get_utf8,
    get_var_u32, MQTTCodecError, PROP_SIZE_U16, PROP_SIZE_U32, PROP_SIZE_U8,
};
use crate::property::{PropertyEncode, PropertySize};
use crate::{
    put_utf8, put_var_u32, variable_byte_int_size, Decode, Encode, FixedHeader, PacketType,
    PropertyType, QoSLevel, Size, UserPropertyMap, WillMessage,
};
use bytes::{Buf, BufMut, BytesMut};
use prop_macro::{PropertyEncode, PropertySize};
use std::collections::HashSet;

const MQTT_PROTOCOL_NAME_LEN: u16 = 0x00_04;
const MQTT_PROTOCOL_U32: u32 = 0x4d515454;
const MQTT_PROTOCOL_VERSION: u8 = 0x05;

pub(crate) const CONNECT_FLAG_USERNAME: u8 = 0b_1000_0000;
pub(crate) const CONNECT_FLAG_PASSWORD: u8 = 0b_0100_0000;
pub(crate) const CONNECT_FLAG_WILL_RETAIN: u8 = 0b_0010_0000;
pub(crate) const CONNECT_FLAG_WILL_QOS: u8 = 0b_0001_1000;
pub(crate) const CONNECT_FLAG_WILL: u8 = 0b_0000_0100;
pub(crate) const CONNECT_FLAG_CLEAN_START: u8 = 0b_0000_0010;
pub(crate) const CONNECT_FLAG_SHIFT: u8 = 0x03;

const DEFAULT_RECEIVE_MAX: u16 = 0xffff;
/// Default remaining size for connect packet
const DEFAULT_CONNECT_REMAINING: u32 = 10;

#[derive(PropertyEncode, PropertySize, Debug, Clone, Eq, PartialEq)]
pub struct Connect {
    pub clean_start: bool,
    pub keep_alive: u16,
    pub session_expiry_interval: Option<u32>,
    pub receive_max: u16,
    pub max_packet_size: Option<u32>,
    pub topic_alias_max: Option<u16>,
    pub req_resp_info: bool,
    pub problem_info: bool,
    pub auth_method: Option<String>,
    pub auth_data: Option<Vec<u8>>,
    pub will_message: Option<WillMessage>,
    pub user_props: Option<UserPropertyMap>,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<Vec<u8>>,
}

impl Connect {
    fn encode_flags(&self, dest: &mut BytesMut) {
        let mut flags = 0_u8;
        if self.clean_start {
            flags |= CONNECT_FLAG_CLEAN_START;
        }
        if self.username.is_some() {
            flags |= CONNECT_FLAG_USERNAME;
        }
        if self.password.is_some() {
            flags |= CONNECT_FLAG_PASSWORD;
        }
        if let Some(will) = &self.will_message {
            flags |= CONNECT_FLAG_WILL;
            if will.retain {
                flags |= CONNECT_FLAG_WILL_RETAIN
            }
            flags |= (will.qos as u8) << CONNECT_FLAG_SHIFT;
        }
        dest.put_u8(flags);
    }

    pub fn decode(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError> {
        let len = src.get_u16();
        if len != 0x04 {
            return Err(MQTTCodecError::new(
                format!("invalid protocol name length: {}", len).as_str(),
            ));
        }
        let mqtt_str = src.get_u32();
        if mqtt_str != MQTT_PROTOCOL_U32 {
            return Err(MQTTCodecError::new("unsupported protocol"));
        }
        let protocol_version = src.get_u8();
        if protocol_version != MQTT_PROTOCOL_VERSION {
            return Err(MQTTCodecError::new(
                format!("unsupported protocol version: {}", protocol_version).as_str(),
            ));
        }
        // connect flags
        let connect_flags = src.get_u8();
        let username = connect_flags & CONNECT_FLAG_USERNAME != 0;
        let password = connect_flags & CONNECT_FLAG_PASSWORD != 0;
        self.clean_start = connect_flags & CONNECT_FLAG_CLEAN_START != 0;
        if connect_flags & CONNECT_FLAG_WILL != 0 {
            let will_retain = connect_flags & CONNECT_FLAG_WILL_RETAIN != 0;
            let qos = connect_flags & CONNECT_FLAG_WILL_QOS >> CONNECT_FLAG_SHIFT;
            if let Ok(qos) = QoSLevel::try_from(qos) {
                self.will_message = Some(WillMessage::new(qos, will_retain));
            } else {
                return Err(MQTTCodecError::new("invalid Will QoS level"));
            }
        }
        self.keep_alive = src.get_u16();
        self.decode_properties(src)?;
        self.decode_payload(src, username, password)?;
        Ok(())
    }

    fn decode_properties(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError> {
        let prop_size = get_var_u32(src);
        let read_until = src.remaining() - prop_size as usize;
        let mut properties: HashSet<PropertyType> = HashSet::new();
        while src.remaining() > read_until {
            match PropertyType::try_from(src.get_u8()) {
                Ok(property_type) => {
                    if property_type != PropertyType::UserProperty {
                        check_property(property_type, &mut properties)?;
                        match property_type {
                            PropertyType::SessionExpiryInt => {
                                self.session_expiry_interval = Some(src.get_u32());
                            }
                            PropertyType::RecvMax => {
                                self.receive_max = src.get_u16();
                            }
                            PropertyType::MaxPacketSize => {
                                self.max_packet_size = Some(src.get_u32());
                            }
                            PropertyType::TopicAliasMax => {
                                self.topic_alias_max = Some(src.get_u16());
                            }
                            PropertyType::ReqRespInfo => match src.get_u8() {
                                0 => self.req_resp_info = false,
                                1 => self.req_resp_info = true,
                                v => {
                                    return Err(MQTTCodecError::new(
                                        format!(
                                            "invalid value {} for {}",
                                            v,
                                            PropertyType::ReqRespInfo
                                        )
                                        .as_str(),
                                    ))
                                }
                            },
                            PropertyType::RespInfo => match src.get_u8() {
                                0 => self.problem_info = false,
                                1 => self.problem_info = true,
                                v => {
                                    return Err(MQTTCodecError::new(
                                        format!(
                                            "invalid value {} for {}",
                                            v,
                                            PropertyType::RespInfo
                                        )
                                        .as_str(),
                                    ))
                                }
                            },
                            PropertyType::AuthMethod => {
                                let result = get_utf8(src)?;
                                self.auth_method = Some(result);
                            }
                            PropertyType::AuthData => {
                                if self.auth_method.is_some() {
                                    self.auth_data = Some(get_bin(src)?);
                                } else {
                                    // MQTT protocol not specific that auth method must appear before
                                    // auth data. This implementation imposes order and may be incorrect
                                    return Err(MQTTCodecError::new(&format!(
                                        "Property: {} provided without providing {}",
                                        PropertyType::AuthData,
                                        PropertyType::AuthMethod
                                    )));
                                }
                            }
                            val => {
                                return Err(MQTTCodecError::new(&format!(
                                    "unexpected property type value: {}",
                                    val
                                )))
                            }
                        }
                    } else {
                        if self.user_props.is_none() {
                            self.user_props = Some(UserPropertyMap::default());
                        }
                        let property_map = self.user_props.as_mut().unwrap();
                        let key = get_utf8(src)?;
                        let value = get_utf8(src)?;
                        property_map.add_property(&key, &value);
                    }
                }
                Err(e) => return Err(e),
            };
            if src.remaining() < read_until {
                return Err(MQTTCodecError::new(
                    "property size does not match expected length",
                ));
            }
        }
        Ok(())
    }

    fn decode_payload(
        &mut self,
        src: &mut BytesMut,
        username: bool,
        password: bool,
    ) -> Result<(), MQTTCodecError> {
        self.client_id = get_utf8(src)?;
        if self.will_message.is_some() {
            let will_message = self.will_message.as_mut().unwrap();
            will_message.decode(src)?;
        }
        if username {
            self.username = Some(get_utf8(src)?);
        }
        if password {
            self.password = Some(get_bin(src)?);
        }
        Ok(())
    }
}

impl crate::Size for Connect {
    fn size(&self) -> u32 {
        let property_remaining = self.property_size();
        let len = variable_byte_int_size(property_remaining);
        DEFAULT_CONNECT_REMAINING + len + property_remaining + self.payload_size()
    }

    fn property_size(&self) -> u32 {
        let mut property_remaining = 0;
        if self.session_expiry_interval.is_some() {
            property_remaining += PROP_SIZE_U32;
        }
        // protocol defaults to 65536 if not included
        if self.receive_max != DEFAULT_RECEIVE_MAX {
            property_remaining += PROP_SIZE_U16;
        }
        if self.max_packet_size.is_some() {
            property_remaining += PROP_SIZE_U32;
        }
        if self.topic_alias_max.is_some() {
            property_remaining += PROP_SIZE_U16;
        }
        // protocol defaults to false if not included
        if self.req_resp_info {
            property_remaining += PROP_SIZE_U8;
        }
        // protocol defaults to true if not included
        if !self.problem_info {
            property_remaining += PROP_SIZE_U8
        }
        if let Some(user_property) = self.user_props.as_ref() {
            property_remaining += user_property.size();
        }
        if let Some(auth_method) = self.auth_method.as_ref() {
            property_remaining += 3 + auth_method.len() as u32;
        }
        if let Some(auth_data) = self.auth_data.as_ref() {
            property_remaining += 3 + auth_data.len() as u32;
        }
        property_remaining
    }

    fn payload_size(&self) -> u32 {
        let mut remaining = 2 + self.client_id.len() as u32;
        if let Some(will_message) = &self.will_message {
            remaining += will_message.size();
        }
        if let Some(username) = &self.username {
            remaining += username.len() as u32 + 2;
        }
        if let Some(password) = &self.password {
            remaining += password.len() as u32 + 2;
        }
        remaining
    }
}

impl Encode for Connect {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        let mut header = FixedHeader::new(PacketType::Connect);
        let prop_remaining = self.property_size();
        let payload_remaining = self.payload_size();
        header.set_remaining(
            DEFAULT_CONNECT_REMAINING
                + prop_remaining
                + payload_remaining
                + variable_byte_int_size(prop_remaining),
        );
        header.encode(dest)?;
        dest.put_u16(MQTT_PROTOCOL_NAME_LEN);
        dest.put_u32(MQTT_PROTOCOL_U32);
        dest.put_u8(MQTT_PROTOCOL_VERSION);
        self.encode_flags(dest);
        dest.put_u16(self.keep_alive);
        put_var_u32(prop_remaining, dest);
        if let Some(expiry) = self.session_expiry_interval {
            dest.put_u8(PropertyType::SessionExpiryInt as u8);
            dest.put_u32(expiry);
        }
        if self.receive_max != DEFAULT_RECEIVE_MAX {
            encode_u16_property(PropertyType::RecvMax, self.receive_max, dest);
        }
        if let Some(max_packet_size) = self.max_packet_size {
            encode_u32_property(PropertyType::MaxPacketSize, max_packet_size, dest);
        }
        if let Some(topic_alias_max) = self.topic_alias_max {
            encode_u16_property(PropertyType::TopicAliasMax, topic_alias_max, dest);
        }
        if self.req_resp_info {
            encode_u8_property(PropertyType::ReqRespInfo, 1, dest);
        }
        if !self.problem_info {
            encode_u8_property(PropertyType::ReqProblemInfo, 1, dest);
        }
        if let Some(user_properties) = &self.user_props {
            user_properties.encode(dest)?;
        }
        if let Some(auth_method) = &self.auth_method {
            encode_utf8_property(PropertyType::AuthMethod, auth_method, dest)?;
        }
        if let Some(auth_data) = &self.auth_data {
            encode_bin_property(PropertyType::AuthData, auth_data, dest)?;
        }
        // payload
        put_utf8(&self.client_id, dest)?;
        // connect payload
        if let Some(will_message) = &self.will_message {
            will_message.encode(dest)?;
        }
        if let Some(username) = &self.username {
            put_utf8(username, dest)?;
        }
        if let Some(password) = &self.password {
            put_bin(password, dest)?;
        }
        Ok(())
    }
}

impl Decode for Connect {
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError> {
        self.decode(src)
    }
}

impl Default for Connect {
    fn default() -> Self {
        Connect {
            clean_start: false,
            keep_alive: 0,
            session_expiry_interval: None,
            receive_max: DEFAULT_RECEIVE_MAX,
            max_packet_size: None,
            topic_alias_max: None,
            req_resp_info: false,
            problem_info: true,
            auth_method: None,
            auth_data: None,
            will_message: None,
            user_props: None,
            client_id: "".to_string(),
            username: None,
            password: None,
        }
    }
}
