use crate::codec::{
    check_property, decode_utf8_string, decode_variable_len_integer, encode_variable_len_integer,
    decode_binary_data, MQTTCodecError, PropertyType,
};
use crate::{Decode, Encode, QoSLevel, WillMessage};
use bytes::{Buf, BytesMut};
use std::collections::{HashMap, HashSet};

const MQTT_PROTOCOL_U32: u32 = 0x4d515454;
const MQTT_PROTOCOL_VERSION: u8 = 0x05;

const CONNECT_FLAG_USERNAME: u8 = 0b_1000_0000;
const CONNECT_FLAG_PASSWORD: u8 = 0b_0100_0000;
const CONNECT_FLAG_WILL_RETAIN: u8 = 0b_0010_0000;
const CONNECT_FLAG_WILL_QOS: u8 = 0b_0001_1000;
const CONNECT_FLAG_WILL: u8 = 0b_0000_0100;
const CONNECT_FLAG_CLEAN_START: u8 = 0b_0000_0010;

const CONNECT_FLAG_SHIFT: u8 = 0x03;
const DEFAULT_RECEIVE_MAX: u16 = 0xffff;

#[derive(Debug, Eq, PartialEq)]
pub struct Connect {
    clean_start: bool,
    keep_alive: u16,
    session_expiry_interval: Option<u32>,
    receive_max: u16,
    max_packet_size: Option<u32>,
    topic_alias_max: Option<u16>,
    req_resp_info: bool,
    problem_info: bool,
    auth_method: Option<String>,
    auth_data: Option<Vec<u8>>,
    will_message: Option<WillMessage>,
    user_property: Option<HashMap<String, String>>,
    client_id: String,
    username: Option<String>,
    password: Option<Vec<u8>>,
}

impl Connect {
    pub(crate) fn decode(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError> {
        let mut connect = Connect::default();
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
        connect.clean_start = connect_flags & CONNECT_FLAG_CLEAN_START != 0;
        if connect_flags & CONNECT_FLAG_WILL != 0 {
            let will_retain = connect_flags & CONNECT_FLAG_WILL_RETAIN != 0;
            let qos = connect_flags & CONNECT_FLAG_WILL_QOS >> CONNECT_FLAG_SHIFT;
            if let Ok(qos) = QoSLevel::try_from(qos) {
                connect.will_message = Some(WillMessage::new(qos, will_retain));
            } else {
                return Err(MQTTCodecError::new("invalid Will QoS level"));
            }
        }
        connect.keep_alive = src.get_u16();
        connect.decode_properties(src)?;
        connect.decode_payload(src, username, password)?;
        Ok(())
    }

    fn decode_will_properties(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError> {
        let prop_size = decode_variable_len_integer(src);
        let read_until = src.remaining() - prop_size as usize;
        let mut properties: HashSet<PropertyType> = HashSet::new();
        let will_message = self.will_message.as_mut().unwrap();
        while src.remaining() > read_until {
            match PropertyType::try_from(src.get_u8()) {
                Ok(property_type) => match property_type {
                    PropertyType::WillDelay => {
                        check_property(PropertyType::WillDelay, &mut properties)?;
                        will_message.delay_interval = src.get_u32();
                    }
                    PropertyType::PayloadFormat => {
                        check_property(PropertyType::PayloadFormat, &mut properties)?;
                        match src.get_u8() {
                            0 => will_message.payload_utf8 = false,
                            1 => will_message.payload_utf8 = true,
                            err => return Err(MQTTCodecError::new( &format!(
                                "unexpected will message payload format value: {}",
                               err
                            )))
                        }
                    }
                    PropertyType::MessageExpiry => {
                        check_property(PropertyType::MessageExpiry, &mut properties)?;
                        will_message.expiry_interval = Some(src.get_u32());
                    }
                    PropertyType::ContentType => {
                        check_property(PropertyType::ContentType, &mut properties)?;
                        will_message.content_type = Some(decode_utf8_string(src)?);
                    }
                    PropertyType::ResponseTopic => {
                        check_property(PropertyType::ResponseTopic, &mut properties)?;
                        will_message.response_topic = Some(decode_utf8_string(src)?);
                        will_message.is_request = true;
                    }
                    PropertyType::CorrelationData => {
                        check_property(PropertyType::CorrelationData, &mut properties)?;
                        let mut dest = Vec::<u8>::new();
                        decode_binary_data(&mut dest, src)?;
                        will_message.correlation_data = Some(dest);
                    }
                    PropertyType::UserProperty => {
                        if will_message.user_property == None {
                            will_message.user_property = Some(HashMap::new());
                        }
                        let property_map =
                            will_message.user_property.as_mut().unwrap();
                        let key = decode_utf8_string(src)?;
                        let value = decode_utf8_string(src)?;
                        property_map.insert(key, value);
                    }
                    err => {
                        return Err(MQTTCodecError::new(&format!(
                            "unexpected will property id: {}",
                            err
                        )))
                    }
                },
                Err(e) => {
                    return Err(MQTTCodecError::new(&format!(
                        "unknown property type: {:?}",
                        e
                    )))
                }
            };
        }
        Ok(())
    }

    fn decode_properties(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError> {
        let prop_size = decode_variable_len_integer(src);
        let read_until = src.remaining() - prop_size as usize;
        let mut properties: HashSet<PropertyType> = HashSet::new();
        while src.remaining() > read_until {
            match PropertyType::try_from(src.get_u8()) {
                Ok(property_type) => {
                    match property_type {
                        PropertyType::SessionExpiry => {
                            check_property(PropertyType::SessionExpiry, &mut properties)?;
                            self.session_expiry_interval = Some(src.get_u32());
                        }
                        PropertyType::RecvMax => {
                            check_property(PropertyType::SessionExpiry, &mut properties)?;
                            self.receive_max = src.get_u16();
                        }
                        PropertyType::MaxPacketSize => {
                            check_property(PropertyType::SessionExpiry, &mut properties)?;
                            self.max_packet_size = Some(src.get_u32());
                        }
                        PropertyType::TopicAliasMax => {
                            check_property(PropertyType::SessionExpiry, &mut properties)?;
                            self.topic_alias_max = Some(src.get_u16());
                        }
                        PropertyType::ReqRespInfo => {
                            check_property(PropertyType::ReqRespInfo, &mut properties)?;
                            match src.get_u8() {
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
                            }
                        }
                        PropertyType::RespInfo => {
                            check_property(PropertyType::RespInfo, &mut properties)?;
                            match src.get_u8() {
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
                            }
                        }
                        PropertyType::AuthMethod => {
                            check_property(PropertyType::AuthMethod, &mut properties)?;
                            let result = decode_utf8_string(src)?;
                            self.auth_method = Some(result);
                        }
                        PropertyType::AuthData => {
                            if self.auth_method != None {
                                check_property(PropertyType::AuthData, &mut properties)?;
                                let mut dest = Vec::new();
                                decode_binary_data(&mut dest, src)?;
                                self.auth_data = Some(dest);
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
                        PropertyType::UserProperty => {
                            if self.user_property == None {
                                self.user_property = Some(HashMap::new());
                            }
                            let property_map = self.user_property.as_mut().unwrap();
                            // MQTT v5.0 specification indicates that a key may appear multiple times
                            // this implementation will overwrite existing values with duplicate
                            // keys
                            let key = decode_utf8_string(src)?;
                            let value = decode_utf8_string(src)?;
                            property_map.insert(key, value);
                        }
                        val => {
                            return Err(MQTTCodecError::new(
                                &format!("unexpected property type value: {}", val),
                            ))
                        }
                    }
                }
                Err(_) => return Err(MQTTCodecError::new("invalid property type")),
            };
            if src.remaining() < read_until {
                return Err(MQTTCodecError::new(
                    "property size does not match expected length",
                ));
            }
        }
        Ok(())
    }

    fn decode_payload(&mut self, src: &mut BytesMut, username: bool, password: bool) -> Result<(), MQTTCodecError> {
        if src.remaining() < 3 {
            return Err(MQTTCodecError::new("missing client ID"));
        }
        self.client_id = decode_utf8_string(src)?;
        if self.will_message != None {
            self.decode_will_properties(src)?;
            let will_message = self.will_message.as_mut().unwrap();
            will_message.topic = decode_utf8_string(src)?;
            decode_binary_data(&mut will_message.payload, src)?;
        }
        if username {
            self.username = Some(decode_utf8_string(src)?);
        }
        if password {
            let mut dest: Vec<u8> = Vec::new();
            decode_binary_data(&mut dest, src)?;
            self.password = Some(dest);
        }
        Ok(())
    }
}

impl crate::Sized for Connect {
    fn size(&self) -> u32 {
        todo!()
    }
}

impl Encode for Connect {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        todo!()
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
            user_property: None,
            client_id: "".to_string(),
            username: None,
            password: None,
        }
    }
}
