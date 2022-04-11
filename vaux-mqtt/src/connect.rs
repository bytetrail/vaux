use std::collections::HashMap;
use bytes::{Buf, BytesMut};
use tokio_util::codec::Decoder;
use crate::{AuthData, MQTTCodecError, QoSLevel, WillMessage};


const MQTT_PROTOCOL_U32: u32 = 0x4d515454;
const MQTT_PROTOCOL_VERSION: u8 = 0x05;

const CONNECT_FLAG_USERNAME: u8 = 0b_1000_0000;
const CONNECT_FLAG_PASSWORD: u8 = 0b_0100_0000;
const CONNECT_FLAG_WILL_RETAIN: u8 = 0b_0010_0000;
const CONNECT_FLAG_WILL_QOS: u8 = 0b_0001_1000;
const CONNECT_FLAG_WILL: u8 = 0b_0000_0100;
const CONNECT_FLAG_CLEAN_START: u8 = 0b_0000_0010;

const CONNECT_FLAG_SHIFT: u8 = 0x03;

#[derive(Debug)]
pub struct Connect {
    username: bool,
    password: bool,
    clean_start: bool,
    keep_alive: u16,
    session_expiry_interval: u32,
    receive_max: u16,
    max_packet_size: u32,
    topic_alias_max: u16,
    req_resp_info: bool,
    request_info: bool,
    auth: Option<AuthData>,
    will_message: Option<WillMessage>,
    user_property: Option<HashMap<String, String>>,
}

impl Default for Connect {
    fn default() -> Self {
        Connect{
            username: false,
            password: false,
            clean_start: false,
            keep_alive: 0,
            session_expiry_interval: 0,
            receive_max: 0,
            max_packet_size: 1024,
            topic_alias_max: 10,
            req_resp_info: false,
            request_info: false,
            auth: None,
            will_message: None,
            user_property: None
        }
    }
}

impl Decoder for Connect {
    type Item = Self;
    type Error = MQTTCodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut connect = Connect::default();
        let len = src.get_u16();
        if len != 0x04 {
            return Err(MQTTCodecError::new(format!("invalid protocol name length: {}", len).as_str()));
        }
        if src.get_u32() != MQTT_PROTOCOL_U32 {
            return Err(MQTTCodecError::new("unsupported protocol"));
        }
        let protocol_version = src.get_u8();
        if protocol_version != MQTT_PROTOCOL_VERSION {
            return Err(MQTTCodecError::new(format!("unsupported protocol version: {}", protocol_version).as_str()));
        }
        // connect flags
        let connect_flags = src.get_u8();
        connect.username = if connect_flags & CONNECT_FLAG_USERNAME != 0 { true } else { false};
        connect.password = if connect_flags & CONNECT_FLAG_PASSWORD != 0 { true } else { false};
        connect.clean_start = if connect_flags & CONNECT_FLAG_CLEAN_START != 0{true} else { false};
        if connect_flags & CONNECT_FLAG_WILL != 0 {
            let will_retain = if connect_flags & CONNECT_FLAG_WILL_RETAIN != 0 {true} else { false};
            let qos = connect_flags & CONNECT_FLAG_WILL_QOS >> CONNECT_FLAG_SHIFT;
            if let Ok(qos) = QoSLevel::try_from(qos) {
                connect.will_message = Some(
                    WillMessage::new(qos, will_retain)
                );
            } else {
                return Err(MQTTCodecError::new("invalid Will QoS level"));
            }
        }
        Ok(Some(connect))
    }
}