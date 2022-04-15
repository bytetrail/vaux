use std::collections::{HashMap, HashSet};
use bytes::{Buf, BytesMut};
use tokio_util::codec::Decoder;
use crate::{QoSLevel, WillMessage};
use crate::codec::{check_property, decode_variable_len_integer, encode_variable_len_integer, MQTTCodecError, PropertyType, read_utf8_string};

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

#[derive(Debug)]
pub struct Connect {
    username: bool,
    password: bool,
    clean_start: bool,
    keep_alive: u16,
    session_expiry_interval: Option<u32>,
    receive_max: u16,
    max_packet_size: Option<u32>,
    topic_alias_max: Option<u16>,
    req_resp_info: bool,
    problem_info: bool,
    auth_method: Option<String>,
    auth_data: Option<u8>,
    will_message: Option<WillMessage>,
    user_property: Option<HashMap<String, String>>,

}

impl Connect {
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
                                v => return Err(MQTTCodecError::new(format!("invalid value {} for {}", v, PropertyType::ReqRespInfo).as_str()))
                            }
                        }
                        PropertyType::RespInfo => {
                            check_property(PropertyType::RespInfo, &mut properties)?;
                            match src.get_u8() {
                                0 => self.problem_info = false,
                                1 => self.problem_info = true,
                                v => return Err(MQTTCodecError::new(format!("invalid value {} for {}", v, PropertyType::RespInfo).as_str()))
                            }
                        }
                        PropertyType::AuthMethod => {
                            check_property(PropertyType::AuthMethod, &mut properties)?;
                            let result = read_utf8_string(src)?;
                            self.auth_method = Some(result);
                        }
                        PropertyType::AuthData=> {
                            if self.auth_method != None {
                                check_property(PropertyType::AuthData, &mut properties)?;

                            } else {
                                // MQTT protocol not specific that auth method must appear before
                                // auth data. This implementation imposes order and may be incorrect
                                return Err(MQTTCodecError::new(&format!("Property: {} provided without providing {}", PropertyType::AuthData, PropertyType::AuthMethod)));
                            }
                        }
                        PropertyType::UserProperty => {
                            // TODO read UTF-8 string pair
                        }
                        _ => return Err(MQTTCodecError::new(format!("unexpected property").as_str())),
                    }
                },
                Err(_) => return Err(MQTTCodecError::new("invalid property type"))
            };
            if src.remaining() < read_until {
                return Err(MQTTCodecError::new("property size does not match expected length"))
            }
        }
        Ok(())
    }
}

impl Default for Connect {
    fn default() -> Self {
        Connect{
            username: false,
            password: false,
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
        connect.keep_alive = src.get_u16();
        connect.decode_properties(src)?;
        Ok(Some(connect))
    }
}