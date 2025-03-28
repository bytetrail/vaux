use crate::codec::{get_bin, get_utf8, put_bin, MqttCodecError};
use crate::property::{Property, PropertyBundle};
use crate::{
    put_utf8, variable_byte_int_size, Decode, Encode, FixedHeader, PacketType, PropertyType,
    QoSLevel, Size, WillMessage,
};
use bytes::{Buf, BufMut, BytesMut};
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

/// Default remaining size for connect packet
const DEFAULT_CONNECT_REMAINING: u32 = 10;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Connect {
    props: PropertyBundle,
    pub clean_start: bool,
    pub keep_alive: u16,
    pub will_message: Option<WillMessage>,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<Vec<u8>>,
}

impl Connect {
    pub fn session_expiry(&self) -> Option<u32> {
        self.props
            .get_property(PropertyType::SessionExpiryInterval)
            .and_then(|p| {
                if let Property::SessionExpiryInterval(interval) = p {
                    Some(*interval)
                } else {
                    None
                }
            })
    }

    pub fn set_session_expiry(&mut self, interval: u32) {
        if interval == 0 {
            self.props
                .clear_property(PropertyType::SessionExpiryInterval);
            return;
        }
        self.props
            .set_property(Property::SessionExpiryInterval(interval));
    }

    pub fn properties(&self) -> &PropertyBundle {
        &self.props
    }

    pub fn properties_mut(&mut self) -> &mut PropertyBundle {
        &mut self.props
    }

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

    pub fn decode(&mut self, src: &mut BytesMut) -> Result<(), MqttCodecError> {
        let len = src.get_u16();
        if len != 0x04 {
            return Err(MqttCodecError::new(
                format!("invalid protocol name length: {}", len).as_str(),
            ));
        }
        let mqtt_str = src.get_u32();
        if mqtt_str != MQTT_PROTOCOL_U32 {
            return Err(MqttCodecError::new("unsupported protocol"));
        }
        let protocol_version = src.get_u8();
        if protocol_version != MQTT_PROTOCOL_VERSION {
            return Err(MqttCodecError::new(
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
            let qos = connect_flags & (CONNECT_FLAG_WILL_QOS >> CONNECT_FLAG_SHIFT);
            if let Ok(qos) = QoSLevel::try_from(qos) {
                self.will_message = Some(WillMessage::new(qos, will_retain));
            } else {
                return Err(MqttCodecError::new("invalid Will QoS level"));
            }
        }
        self.keep_alive = src.get_u16();
        if src.remaining() > 0 {
            self.props.decode(src)?;
        }
        self.decode_payload(src, username, password)?;
        Ok(())
    }

    fn decode_payload(
        &mut self,
        src: &mut BytesMut,
        username: bool,
        password: bool,
    ) -> Result<(), MqttCodecError> {
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
        self.props.size()
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
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
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
        self.props.encode(dest)?;
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
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MqttCodecError> {
        self.decode(src)
    }
}

impl Default for Connect {
    fn default() -> Self {
        let mut allowed = HashSet::new();
        allowed.insert(PropertyType::SessionExpiryInterval);
        allowed.insert(PropertyType::RecvMax);
        allowed.insert(PropertyType::MaxPacketSize);
        allowed.insert(PropertyType::TopicAliasMax);
        allowed.insert(PropertyType::ReqRespInfo);
        allowed.insert(PropertyType::ReqProblemInfo);
        allowed.insert(PropertyType::UserProperty);
        allowed.insert(PropertyType::AuthMethod);
        allowed.insert(PropertyType::AuthData);

        Connect {
            props: PropertyBundle::new(allowed),
            clean_start: false,
            keep_alive: 0,
            will_message: None,
            client_id: "".to_string(),
            username: None,
            password: None,
        }
    }
}
