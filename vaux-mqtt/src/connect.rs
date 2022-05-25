use crate::codec::{
    check_property, decode_binary_data, decode_utf8_string, decode_variable_len_integer,
    MQTTCodecError, PropertyType, PROP_SIZE_U16, PROP_SIZE_U32, PROP_SIZE_U8,
};
use crate::{variable_byte_int_size, Decode, Encode, QoSLevel, UserPropertyMap, WillMessage, FixedHeader, PacketType, Remaining};
use bytes::{Buf, BufMut, BytesMut};
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
            flags |= (will.qos as u8 ) << CONNECT_FLAG_SHIFT;
        }
        dest.put_u8(flags);
    }

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
            let will_message = self.will_message.as_mut().unwrap();
            will_message.decode(src)?;
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
        let property_remaining = self.property_remaining().unwrap();
        let len = variable_byte_int_size(property_remaining);
        DEFAULT_CONNECT_REMAINING + len + property_remaining + self.payload_remaining().unwrap()
    }

    fn property_remaining(&self) -> Option<u32> {
        let mut property_remaining = 0;
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
        Some(property_remaining)
    }

    fn payload_remaining(&self) -> Option<u32> {
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
        Some(remaining)
    }
}

impl Encode for Connect {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        let mut header = FixedHeader::new(PacketType::Connect);
        let prop_remaining = self.property_remaining().unwrap();
        let payload_remaining = self.payload_remaining().unwrap();
        header.set_remaining(
                DEFAULT_CONNECT_REMAINING
                + prop_remaining + payload_remaining
                + variable_byte_int_size(prop_remaining)
        );
        header.encode(dest)?;
        dest.put_u32(MQTT_PROTOCOL_U32);
        dest.put_u8(MQTT_PROTOCOL_VERSION);
        self.encode_flags(dest);
        dest.put_u16(self.keep_alive);

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
    fn test_encode_flags() {
        let mut connect= Connect::default();
        connect.clean_start = true;
        let mut dest = BytesMut::new();
        let result = connect.encode(&mut dest);
        assert!(result.is_ok());
        assert_eq!(dest[7], CONNECT_FLAG_CLEAN_START);
        let mut dest = BytesMut::new();
        let password = vec![1,2,3,4,5];
        connect.password = Some(password);
        let result = connect.encode(&mut dest);
        assert!(result.is_ok());
        assert_eq!(dest[7], CONNECT_FLAG_CLEAN_START | CONNECT_FLAG_PASSWORD);
    }

    #[test]
    fn test_encode_will() {
        let mut connect= Connect::default();
        let will = WillMessage::new(QoSLevel::AtLeastOnce, true);
        connect.will_message = Some(will);
        let mut dest = BytesMut::new();
        let result = connect.encode(&mut dest);
        assert!(result.is_ok());
        assert!(dest[7] & CONNECT_FLAG_WILL > 0);
        assert!(dest[7] & CONNECT_FLAG_WILL_RETAIN > 0);
        let qos_bits = (dest[7] & CONNECT_FLAG_WILL_QOS) >> CONNECT_FLAG_SHIFT;
        let qos_result = QoSLevel::try_from(qos_bits);
        assert!(qos_result.is_ok());
        let qos = qos_result.unwrap();
        assert_eq!(QoSLevel::AtLeastOnce, qos);
    }

    #[test]
    fn test_encode_keep_alive() {
        let mut connect= Connect::default();
        connect.keep_alive = 0xcafe;
        let mut dest = BytesMut::new();
        let result = connect.encode(&mut dest);
        assert!(result.is_ok());
        assert_eq!(0xcafe, ((dest[8] as u16) << 8) + dest[9] as u16);
    }

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

    fn test_property (
        connect: Connect,
        dest: &mut BytesMut,
        expected_len: u32,
        expected_prop_len: u32,
        property: PropertyType,
    ) {
        let result = connect.encode(dest);
        assert!(result.is_ok());
        assert_eq!(expected_len, dest.len() as u32);
        assert_eq!(expected_prop_len, connect.property_remaining().unwrap());
        assert_eq!(expected_prop_len as u8, dest[11]);
        assert_eq!(property as u8, dest[12]);
    }

}
