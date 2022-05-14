use crate::codec::{
    check_property, decode_binary_data, decode_utf8_string, decode_variable_len_integer,
    MQTTCodecError, PropertyType, PROP_SIZE_U16, PROP_SIZE_U32, PROP_SIZE_U8,
};
use crate::{variable_byte_int_size, Decode, Encode, QoSLevel, UserPropertyMap, WillMessage};
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
/// Default remaining size for connect packet
const DEFAULT_CONNECT_REMAINING: u32 = 10;

#[derive(Debug, Eq, PartialEq)]
pub struct Connect {
    pub clean_start: bool,
    pub keep_alive: u16,
    pub session_expiry_interval: Option<u32>,
    receive_max: u16,
    max_packet_size: Option<u32>,
    topic_alias_max: Option<u16>,
    req_resp_info: bool,
    problem_info: bool,
    auth_method: Option<String>,
    auth_data: Option<Vec<u8>>,
    will_message: Option<WillMessage>,
    user_property: Option<UserPropertyMap>,
    pub client_id: String,
    username: Option<String>,
    password: Option<Vec<u8>>,
}

impl Connect {
    pub(crate) fn decode(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError> {
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
                            err => {
                                return Err(MQTTCodecError::new(&format!(
                                    "unexpected will message payload format value: {}",
                                    err
                                )))
                            }
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
                        will_message.correlation_data = Some(decode_binary_data(src)?);
                    }
                    PropertyType::UserProperty => {
                        if will_message.user_property == None {
                            will_message.user_property = Some(HashMap::new());
                        }
                        let property_map = will_message.user_property.as_mut().unwrap();
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
                    if property_type != PropertyType::UserProperty {
                        check_property(property_type, &mut properties)?;
                        match property_type {
                            PropertyType::SessionExpiry => {
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
                                let result = decode_utf8_string(src)?;
                                self.auth_method = Some(result);
                            }
                            PropertyType::AuthData => {
                                if self.auth_method != None {
                                    self.auth_data = Some(decode_binary_data(src)?);
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

    fn decode_payload(
        &mut self,
        src: &mut BytesMut,
        username: bool,
        password: bool,
    ) -> Result<(), MQTTCodecError> {
        if src.remaining() < 3 {
            return Err(MQTTCodecError::new("missing client ID"));
        }
        self.client_id = decode_utf8_string(src)?;
        if self.will_message != None {
            self.decode_will_properties(src)?;
            let will_message = self.will_message.as_mut().unwrap();
            will_message.topic = decode_utf8_string(src)?;
            will_message.payload = decode_binary_data(src)?;
        }
        if username {
            self.username = Some(decode_utf8_string(src)?);
        }
        if password {
            self.password = Some(decode_binary_data(src)?);
        }
        Ok(())
    }
}

impl crate::Remaining for Connect {
    fn size(&self) -> u32 {
        let mut remaining = DEFAULT_CONNECT_REMAINING;
        // property sizes, add variable len integer for property size
        let mut property_remaining = 0_u32;
        if self.session_expiry_interval != None {
            property_remaining += PROP_SIZE_U32;
        }
        // protocol defaults to 65536 if not included
        if self.receive_max != DEFAULT_RECEIVE_MAX {
            property_remaining += PROP_SIZE_U16;
        }
        if self.max_packet_size != None {
            property_remaining += PROP_SIZE_U32;
        }
        if self.topic_alias_max != None {
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
        if let Some(user_property) = self.user_property.as_ref() {
            property_remaining += user_property.size();
        }
        if let Some(auth_method) = self.auth_method.as_ref() {
            property_remaining += 3 + auth_method.len() as u32;
        }
        if let Some(auth_data) = self.auth_data.as_ref() {
            property_remaining += 3 + auth_data.len() as u32;
        }
        // patch up property len
        let len = variable_byte_int_size(property_remaining);
        remaining += len + property_remaining;
        // calculate payload
        remaining += self.payload_remaining().unwrap();
        remaining
    }

    fn property_remaining(&self) -> Option<u32> {
        None
    }

    fn payload_remaining(&self) -> Option<u32> {
        let mut remaining = 2 + self.client_id.len() as u32;
        if let Some(will_message) = &self.will_message {
            remaining += will_message.size();
            remaining += will_message.topic.len() as u32 + 2;
            remaining += will_message.payload.len() as u32 + 2;
        }
        if let Some(username) = &self.username {
            remaining += username.len() as u32 + 2;
        }
        if let Some(password) = &self.password {
            remaining += password.len() as u32 + 2;
        }
        Some(remaining)
    }
}

impl Encode for Connect {
    fn encode(&self, _dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::{Remaining, UserPropertyMap};

    const CONNECT_MIN_REMAINING: u32 = 13;
    const PROP_ENCODE: u32 = 5;

    #[test]
    fn test_default_remaining() {
        let connect = Connect::default();
        let remaining = connect.size();
        assert_eq!(
            CONNECT_MIN_REMAINING, remaining,
            "[Default] expected {} remaining size",
            CONNECT_MIN_REMAINING
        );
    }

    #[test]
    fn test_receive_max_remaining() {
        let mut connect = Connect::default();
        connect.receive_max = 1024;
        let remaining = connect.size();
        let expected = CONNECT_MIN_REMAINING + 3;
        assert_eq!(
            expected, remaining,
            "[Receive Max] expected {} remaining size",
            expected
        );
    }

    #[test]
    fn test_problem_info_remaining() {
        let mut connect = Connect::default();
        connect.problem_info = false;
        let remaining = connect.size();
        let expected = CONNECT_MIN_REMAINING + PROP_SIZE_U8;
        assert_eq!(
            expected, remaining,
            "[Problem Info false] expected {} remaining size",
            expected
        );
        connect.problem_info = true;
        let remaining = connect.size();
        assert_eq!(
            CONNECT_MIN_REMAINING, remaining,
            "[Problem Info true] {} remaining size",
            CONNECT_MIN_REMAINING
        );
    }

    #[test]
    fn test_user_property_remaining() {
        let mut connect = Connect::default();

        let mut user_properties = UserPropertyMap::new();
        let key = "12335".to_string();
        let value = "12345".to_string();
        let expected = CONNECT_MIN_REMAINING + key.len() as u32 + value.len() as u32 + PROP_ENCODE;
        user_properties.insert(key, value);
        connect.user_property = Some(user_properties);
        let remaining = connect.size();
        assert_eq!(
            expected, remaining,
            "[Single Property] expected {} remaining size",
            expected
        );

        let mut user_properties = UserPropertyMap::new();
        let key = "12335".to_string();
        let value = "12345".to_string();
        let mut expected =
            CONNECT_MIN_REMAINING + key.len() as u32 + value.len() as u32 + PROP_ENCODE;
        user_properties.insert(key, value);
        let key = "567890".to_string();
        let value = "567890".to_string();
        expected += key.len() as u32 + value.len() as u32 + PROP_ENCODE;
        user_properties.insert(key, value);
        connect.user_property = Some(user_properties);
        let remaining = connect.size();
        assert_eq!(
            expected, remaining,
            "[2 Properties] expected {} remaining size",
            expected
        );
    }

    /// Remaining size calculations for a connect packet with will message present.
    #[test]
    fn test_will_message_remaining() {
        let mut connect = Connect::default();
        let will_message = WillMessage::new(QoSLevel::AtLeastOnce, true);
        connect.will_message = Some(will_message);
        let remaining = connect.size();
        assert_eq!(
            CONNECT_MIN_REMAINING + 5,
            remaining,
            "[Min Will Message] expected {}",
            CONNECT_MIN_REMAINING + 5
        );
    }
}
