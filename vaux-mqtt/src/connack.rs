use crate::codec::{
    check_property, decode_binary_data, decode_prop_bool, decode_utf8_string,
    decode_variable_len_integer, encode_binary_data, encode_utf8_string,
    encode_variable_len_integer, PropertyType, Reason, PROP_SIZE_U16,
};
use crate::{
    variable_byte_int_size, Decode, Encode, QoSLevel, Remaining, UserPropertyMap, PROP_SIZE_U32,
    PROP_SIZE_U8,
};
use crate::{FixedHeader, MQTTCodecError, PacketType};
use bytes::{Buf, BufMut, BytesMut};
use std::collections::HashSet;

const DEFAULT_RECV_MAX: u16 = 65535;
const DEFAULT_TOPIC_ALIAS_MAX: u16 = 0;
const VARIABLE_HEADER_LEN: u32 = 2;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ConnAck {
    pub session_present: bool,
    reason: Reason,
    reason_str: Option<String>,
    pub expiry_interval: Option<u32>,
    receive_max: u16,
    max_qos: QoSLevel,
    retain_avail: bool,
    max_packet_size: Option<u32>,
    pub assigned_client_id: Option<String>,
    topic_alias_max: u16,
    user_properties: Option<UserPropertyMap>,
    wildcard_sub_avail: bool,
    sub_id_avail: bool,
    shared_sub_avail: bool,
    pub server_keep_alive: Option<u16>,
    response_info: Option<String>,
    server_ref: Option<String>,
    auth_method: Option<String>,
    auth_data: Option<Vec<u8>>,
}

impl ConnAck {
    fn decode_properties(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError> {
        let prop_size = decode_variable_len_integer(src);
        let read_until = src.remaining() - prop_size as usize;
        let mut properties: HashSet<PropertyType> = HashSet::new();
        while src.remaining() > read_until {
            match PropertyType::try_from(src.get_u8()) {
                Ok(property_type) => {
                    if property_type != PropertyType::UserProperty {
                        check_property(property_type, &mut properties)?;
                        self.decode_property(property_type, src)?;
                    } else {
                        if self.user_properties == None {
                            self.user_properties = Some(UserPropertyMap::new());
                        }
                        let property_map = self.user_properties.as_mut().unwrap();
                        let key = decode_utf8_string(src)?;
                        let value = decode_utf8_string(src)?;
                        property_map.add_property(&key, &value);
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

    fn decode_property(
        &mut self,
        property_type: PropertyType,
        src: &mut BytesMut,
    ) -> Result<(), MQTTCodecError> {
        match property_type {
            PropertyType::SessionExpiryInt => self.expiry_interval = Some(src.get_u32()),
            PropertyType::RecvMax => self.receive_max = src.get_u16(),
            PropertyType::MaxQoS => self.max_qos = QoSLevel::try_from(src.get_u8())?,
            PropertyType::RetainAvail => self.retain_avail = decode_prop_bool(src)?,
            PropertyType::MaxPacketSize => self.max_packet_size = Some(src.get_u32()),
            PropertyType::AssignedClientId => {
                self.assigned_client_id = Some(decode_utf8_string(src)?)
            }
            PropertyType::TopicAliasMax => self.topic_alias_max = src.get_u16(),
            PropertyType::Reason => self.reason_str = Some(decode_utf8_string(src)?),
            PropertyType::WildcardSubAvail => self.wildcard_sub_avail = decode_prop_bool(src)?,
            PropertyType::SubIdAvail => self.sub_id_avail = decode_prop_bool(src)?,
            PropertyType::ShardSubAvail => self.shared_sub_avail = decode_prop_bool(src)?,
            PropertyType::KeepAlive => self.server_keep_alive = Some(src.get_u16()),
            PropertyType::RespInfo => self.response_info = Some(decode_utf8_string(src)?),
            PropertyType::ServerRef => self.server_ref = Some(decode_utf8_string(src)?),
            PropertyType::AuthMethod => self.auth_method = Some(decode_utf8_string(src)?),
            PropertyType::AuthData => self.auth_data = Some(decode_binary_data(src)?),
            prop => {
                return Err(MQTTCodecError::new(&format!(
                    "unexpected property type value: {}",
                    prop
                )))
            }
        }
        Ok(())
    }
}

impl Default for ConnAck {
    fn default() -> Self {
        ConnAck {
            session_present: false,
            reason: Reason::Success,
            reason_str: None,
            expiry_interval: None,
            receive_max: DEFAULT_RECV_MAX,
            max_qos: QoSLevel::ExactlyOnce,
            retain_avail: true,
            max_packet_size: None,
            assigned_client_id: None,
            topic_alias_max: DEFAULT_TOPIC_ALIAS_MAX,
            user_properties: None,
            wildcard_sub_avail: true,
            sub_id_avail: true,
            shared_sub_avail: true,
            server_keep_alive: None,
            response_info: None,
            server_ref: None,
            auth_method: None,
            auth_data: None,
        }
    }
}

impl crate::Remaining for ConnAck {
    fn size(&self) -> u32 {
        // variable header is 3 bytes
        3 + self.property_remaining().unwrap()
    }

    fn property_remaining(&self) -> Option<u32> {
        let mut remaining = if self.expiry_interval.is_some() {
            PROP_SIZE_U32
        } else {
            0
        } + if self.receive_max != DEFAULT_RECV_MAX {
            PROP_SIZE_U16
        } else {
            0
        } + if self.max_qos != QoSLevel::ExactlyOnce {
            PROP_SIZE_U8
        } else {
            0
        } + if !self.retain_avail { PROP_SIZE_U8 } else { 0 }
            + if self.max_packet_size.is_some() {
                PROP_SIZE_U32
            } else {
                0
            }
            + self
                .assigned_client_id
                .as_ref()
                .map_or(0, |i| 3 + i.len() as u32)
            + if self.topic_alias_max != DEFAULT_TOPIC_ALIAS_MAX {
                PROP_SIZE_U16
            } else {
                0
            };
        remaining += self.reason_str.as_ref().map_or(0, |r| 3 + r.len() as u32);
        remaining += self.user_properties.as_ref().map_or(0, |p| p.size());
        if !self.wildcard_sub_avail {
            remaining += PROP_SIZE_U8;
        }
        if !self.sub_id_avail {
            remaining += PROP_SIZE_U8;
        }
        if !self.shared_sub_avail {
            remaining += PROP_SIZE_U8;
        }
        if self.server_keep_alive.is_some() {
            remaining += PROP_SIZE_U16;
        }
        remaining += self
            .response_info
            .as_ref()
            .map_or(0, |r| 3 + r.len() as u32)
            + self.server_ref.as_ref().map_or(0, |r| 3 + r.len() as u32)
            + self.auth_method.as_ref().map_or(0, |a| 3 + a.len() as u32)
            + self.auth_data.as_ref().map_or(0, |a| 3 + a.len() as u32);
        Some(remaining)
    }

    /// Implementation of PacketSize. CONNACK packet does not have a payload.
    fn payload_remaining(&self) -> Option<u32> {
        None
    }
}

impl Decode for ConnAck {
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError> {
        self.session_present = (0x01 & src.get_u8()) > 0;
        if let Ok(reason) = src.get_u8().try_into() {
            self.reason = reason;
        }
        self.decode_properties(src)?;
        Ok(())
    }
}

impl Encode for ConnAck {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        let mut header = FixedHeader::new(PacketType::ConnAck);
        let prop_remaining = self.property_remaining().unwrap();
        header.set_remaining(
            VARIABLE_HEADER_LEN + prop_remaining + variable_byte_int_size(prop_remaining),
        );
        header.encode(dest)?;
        let connack_flag: u8 = if self.session_present { 0x01 } else { 0x00 };
        dest.put_u8(connack_flag);
        dest.put_u8(self.reason as u8);
        // reserve capacity to avoid intermediate reallocation
        dest.reserve(prop_remaining as usize);
        encode_variable_len_integer(prop_remaining, dest);
        if let Some(expiry) = self.expiry_interval {
            dest.put_u8(PropertyType::SessionExpiryInt as u8);
            dest.put_u32(expiry);
        }
        if self.receive_max != DEFAULT_RECV_MAX {
            dest.put_u8(PropertyType::RecvMax as u8);
            dest.put_u16(self.receive_max);
        }
        if self.max_qos != QoSLevel::ExactlyOnce {
            dest.put_u8(PropertyType::MaxQoS as u8);
            dest.put_u8(self.max_qos as u8);
        }
        if !self.retain_avail {
            dest.put_u8(PropertyType::RetainAvail as u8);
            dest.put_u8(0);
        }
        if let Some(max_packet_size) = self.max_packet_size {
            dest.put_u8(PropertyType::MaxPacketSize as u8);
            dest.put_u32(max_packet_size);
        }
        if let Some(client_id) = &self.assigned_client_id {
            dest.put_u8(PropertyType::AssignedClientId as u8);
            encode_utf8_string(&client_id, dest)?;
        }
        if self.topic_alias_max != 0 {
            dest.put_u8(PropertyType::TopicAliasMax as u8);
            dest.put_u16(self.topic_alias_max);
        }
        if let Some(reason) = &self.reason_str {
            dest.put_u8(PropertyType::Reason as u8);
            encode_utf8_string(reason, dest)?;
        }
        if let Some(user_properties) = &self.user_properties {
            user_properties.encode(dest)?;
        }
        if !self.wildcard_sub_avail {
            dest.put_u8(PropertyType::WildcardSubAvail as u8);
            dest.put_u8(0);
        }
        if !self.sub_id_avail {
            dest.put_u8(PropertyType::SubIdAvail as u8);
            dest.put_u8(0);
        }
        if !self.shared_sub_avail {
            dest.put_u8(PropertyType::ShardSubAvail as u8);
            dest.put_u8(0);
        }
        if let Some(keep_alive) = self.server_keep_alive {
            dest.put_u8(PropertyType::KeepAlive as u8);
            dest.put_u16(keep_alive);
        }
        if let Some(response) = &self.response_info {
            dest.put_u8(PropertyType::RespInfo as u8);
            encode_utf8_string(response, dest)?;
        }
        if let Some(server_ref) = &self.server_ref {
            dest.put_u8(PropertyType::ServerRef as u8);
            encode_utf8_string(server_ref, dest)?;
        }
        if let Some(auth_method) = &self.auth_method {
            dest.put_u8(PropertyType::AuthMethod as u8);
            encode_utf8_string(auth_method, dest)?;
        }
        if let Some(auth_data) = &self.auth_data {
            dest.put_u8(PropertyType::AuthData as u8);
            encode_binary_data(auth_data, dest)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::codec::PropertyType;
    use crate::PropertyType::MaxQoS;
    use crate::{Encode, UserPropertyMap};
    use bytes::BytesMut;

    /// Minimum length CONNACK return
    /// Byte 1 = packet type + flags
    /// Byte 2 = 1 byte variable byte integer remaining length
    /// Byte 3 = CONNACK flags
    /// Byte 4 = Reason
    /// Byte 5 = 1 byte variable byte integer property length
    const EXPECTED_MIN_CONNACK_LEN: usize = 5;

    #[test]
    fn test_complex_remaining() {
        let reason = "Malformed Packet".to_string();
        let auth_data = vec![0, 1, 2, 3, 4, 5];
        let expected_len = (reason.len() + auth_data.len() + 11) as u32;
        let mut connack = ConnAck::default();
        connack.reason = Reason::MalformedPacket;
        connack.reason_str = Some(reason);
        connack.shared_sub_avail = false;
        connack.auth_data = Some(auth_data);
        assert_eq!(expected_len, connack.size());
    }

    #[test]
    fn test_encode_session_expiry_interval() {
        const EXPECTED_LEN: u32 = EXPECTED_MIN_CONNACK_LEN as u32 + 5;
        const EXPECTED_PROP_LEN: u32 = 5;
        let mut dest = BytesMut::new();
        let mut connack = ConnAck::default();
        connack.expiry_interval = Some(257);
        test_property(
            connack,
            &mut dest,
            EXPECTED_LEN,
            EXPECTED_PROP_LEN,
            PropertyType::SessionExpiryInt,
        );
        // 0x00000101 in bytes 6-9
        assert_eq!(1, dest[8]);
        assert_eq!(1, dest[9]);
    }

    #[test]
    fn test_decode_session_expiry_interval() {
        const EXPECTED_SESSION_EXPIRY: u32 = 0x00001010;
        let encoded = [
            PacketType::ConnAck as u8,
            0x08,
            0x00,
            0x00,
            0x05,
            PropertyType::SessionExpiryInt as u8,
            0x00,
            0x00,
            0x10,
            0x10,
        ];
        let mut connack = ConnAck::default();
        let mut buf = BytesMut::from(&encoded[..]);
        buf.advance(2);
        let result = connack.decode(&mut buf);
        assert!(result.is_ok());
        assert!(connack.expiry_interval.is_some());
        assert_eq!(EXPECTED_SESSION_EXPIRY, connack.expiry_interval.unwrap());
    }

    #[test]
    fn test_encode_recv_max() {
        const EXPECTED_LEN: u32 = EXPECTED_MIN_CONNACK_LEN as u32 + 3;
        const EXPECTED_PROP_LEN: u32 = 3;
        let mut dest = BytesMut::new();
        let mut connack = ConnAck::default();
        connack.receive_max = 257;
        test_property(
            connack,
            &mut dest,
            EXPECTED_LEN,
            EXPECTED_PROP_LEN,
            PropertyType::RecvMax,
        );
        assert_eq!(1, dest[6]);
        assert_eq!(1, dest[7]);
    }

    #[test]
    fn test_decode_recv_max() {
        const EXPECTED_RECEIVE_MAX: u16 = 0x1234;
        let encoded = [
            PacketType::ConnAck as u8,
            0x06,
            0x00,
            0x00,
            0x03,
            PropertyType::RecvMax as u8,
            0x12,
            0x34,
        ];
        let mut connack = ConnAck::default();
        let mut buf = BytesMut::from(&encoded[..]);
        buf.advance(2);
        let result = connack.decode(&mut buf);
        assert!(result.is_ok());
        assert_eq!(EXPECTED_RECEIVE_MAX, connack.receive_max);
    }

    #[test]
    fn test_encode_max_qos() {
        const EXPECTED_LEN: u32 = EXPECTED_MIN_CONNACK_LEN as u32 + 2;
        const EXPECTED_PROP_LEN: u32 = 2;
        let mut dest = BytesMut::new();
        let mut connack = ConnAck::default();
        connack.max_qos = QoSLevel::AtLeastOnce;
        test_property(
            connack,
            &mut dest,
            EXPECTED_LEN,
            EXPECTED_PROP_LEN,
            PropertyType::MaxQoS,
        );
        assert_eq!(QoSLevel::AtLeastOnce as u8, dest[6]);
    }

    #[test]
    fn test_decode_max_qos() {
        const EXPECTED_MAX_QOS: QoSLevel = QoSLevel::ExactlyOnce;
        let encoded = [
            PacketType::ConnAck as u8,
            0x05,
            0x00,
            0x00,
            0x02,
            PropertyType::MaxQoS as u8,
            QoSLevel::ExactlyOnce as u8,
        ];
        let mut connack = ConnAck::default();
        let mut buf = BytesMut::from(&encoded[..]);
        buf.advance(2);
        let result = connack.decode(&mut buf);
        assert!(result.is_ok());
        assert_eq!(EXPECTED_MAX_QOS, connack.max_qos);
    }

    #[test]
    fn test_retain_avail() {
        const EXPECTED_LEN: u32 = EXPECTED_MIN_CONNACK_LEN as u32 + 2;
        const EXPECTED_PROP_LEN: u32 = 2;
        let mut dest = BytesMut::new();
        let mut connack = ConnAck::default();
        connack.retain_avail = false;
        test_property(
            connack,
            &mut dest,
            EXPECTED_LEN,
            EXPECTED_PROP_LEN,
            PropertyType::RetainAvail,
        );
        assert_eq!(0x00, dest[6]);
    }

    #[test]
    fn test_max_packet_size() {
        const EXPECTED_LEN: usize = EXPECTED_MIN_CONNACK_LEN + 5;
        const EXPECTED_PROP_LEN: u32 = 5;
        let mut connack = ConnAck::default();
        connack.max_packet_size = Some(100);
        let mut dest = BytesMut::new();
        test_property(
            connack,
            &mut dest,
            EXPECTED_LEN as u32,
            EXPECTED_PROP_LEN,
            PropertyType::MaxPacketSize,
        );
    }

    #[test]
    fn test_assigned_client_id() {
        const EXPECTED_LEN: u32 = EXPECTED_MIN_CONNACK_LEN as u32 + 43;
        const EXPECTED_PROP_LEN: u32 = 43;
        let mut connack = ConnAck::default();
        let assigned_client_id = format!("{:*^1$}", "", 40);
        connack.assigned_client_id = Some(assigned_client_id);
        let mut dest = BytesMut::new();
        test_property(
            connack,
            &mut dest,
            EXPECTED_LEN,
            EXPECTED_PROP_LEN,
            PropertyType::AssignedClientId,
        );
        assert_eq!(40, dest[7]);
        for ch in &dest[8..48] {
            assert_eq!(0x2a, *ch);
        }
    }

    #[test]
    fn test_encode_topic_alias_max() {
        const EXPECTED_LEN: u32 = EXPECTED_MIN_CONNACK_LEN as u32 + PROP_SIZE_U16;
        const EXPECTED_PROP_LEN: u32 = PROP_SIZE_U16;
        let mut dest = BytesMut::new();
        let mut connack = ConnAck::default();
        connack.topic_alias_max = 128;
        test_property(
            connack,
            &mut dest,
            EXPECTED_LEN,
            EXPECTED_PROP_LEN,
            PropertyType::TopicAliasMax,
        );
        assert_eq!(0x80, dest[7]);
    }

    #[test]
    fn test_decode_topic_alias_max() {
        const EXPECTED_TOPIC_ALIAS_MAX: u16 = 128;
        let encoded = [
            PacketType::ConnAck as u8,
            0x06,
            0x00,
            0x00,
            0x03,
            PropertyType::TopicAliasMax as u8,
            0x00,
            EXPECTED_TOPIC_ALIAS_MAX as u8,
        ];
        let mut connack = ConnAck::default();
        let mut buf = BytesMut::from(&encoded[..]);
        buf.advance(2);
        let result = connack.decode(&mut buf);
        assert!(result.is_ok());
        assert_eq!(EXPECTED_TOPIC_ALIAS_MAX, connack.topic_alias_max);
    }

    #[test]
    fn test_encode_reason_string() {
        let reason = "01234567890123456789".to_string();
        let expected_len = EXPECTED_MIN_CONNACK_LEN as u32 + 3 + reason.len() as u32;
        let expected_prop_len = 3 + reason.len() as u32;
        let mut connack = ConnAck::default();
        connack.reason = Reason::Banned;
        connack.reason_str = Some(reason);
        let mut dest = BytesMut::new();
        test_property(
            connack,
            &mut dest,
            expected_len,
            expected_prop_len,
            PropertyType::Reason,
        );
        assert_eq!(0x14, dest[7]);
    }

    #[test]
    fn test_decode_reason_string() {
        let mut buf = BytesMut::new();
        let reason = "01234567890123456789".to_string();
        let mut connack = ConnAck::default();
        connack.reason_str = Some(reason.clone());
        assert!(connack.encode(&mut buf).is_ok());
        let mut connack = ConnAck::default();
        buf.advance(2);
        let result = connack.decode(&mut buf);
        assert!(result.is_ok());
        assert!(connack.reason_str.is_some());
        assert_eq!(reason.len(), connack.reason_str.unwrap().len());
    }

    #[test]
    fn test_encode_user_properties() {
        let mut properties = UserPropertyMap::new();
        properties.add_property("broker_id", "vaux");
        properties.add_property("broker_version", "0.1.0");
        let mut prop_len = 0;
        for (k, value) in properties.map() {
            let key_len = k.len() as u32 + 2;
            for v in value {
                prop_len += key_len + v.len() as u32 + 3;
            }
        }
        println!("Property len() {}", prop_len);
        println!("User property remaining {}", properties.size());
        let expected_len = EXPECTED_MIN_CONNACK_LEN as u32 + prop_len as u32;
        let expected_prop_len = prop_len as u32;
        let mut dest = BytesMut::new();
        let mut connack = ConnAck::default();
        connack.user_properties = Some(properties);
        test_property(
            connack,
            &mut dest,
            expected_len,
            expected_prop_len,
            PropertyType::UserProperty,
        );
    }

    #[test]
    fn test_decode_user_properties() {
        let mut properties = UserPropertyMap::new();
        properties.add_property("broker_id", "vaux");
        properties.add_property("broker_version", "0.1.0");
        let mut connack = ConnAck::default();
        connack.user_properties = Some(properties);
        let mut buf = BytesMut::new();
        let result = connack.encode(&mut buf);
        assert!(result.is_ok());
        // decode - advance past fixed header
        buf.advance(2);
        let mut connack = ConnAck::default();
        let result = connack.decode(&mut buf);
        assert!(result.is_ok());
        assert!(connack.user_properties.is_some());
        let props = connack.user_properties.unwrap();
        assert!(props.contains_key("broker_id"));
        assert!(props.contains_key("broker_version"));
    }

    #[test]
    fn test_wildcard_sub_avail() {
        let mut connack = ConnAck::default();
        connack.wildcard_sub_avail = false;
        test_bool_property(connack, PropertyType::WildcardSubAvail, false);
    }

    #[test]
    fn test_sub_id_avail() {
        let mut connack = ConnAck::default();
        connack.sub_id_avail = false;
        test_bool_property(connack, PropertyType::SubIdAvail, false);
    }

    #[test]
    fn test_shared_sub_avail() {
        let mut connack = ConnAck::default();
        connack.shared_sub_avail = false;
        test_bool_property(connack, PropertyType::ShardSubAvail, false);
    }

    #[test]
    fn test_encode_server_keep_alive() {
        const EXPECTED_LEN: u32 = EXPECTED_MIN_CONNACK_LEN as u32 + PROP_SIZE_U16;
        const EXPECTED_PROP_LEN: u32 = PROP_SIZE_U16;
        let mut dest = BytesMut::new();
        let mut connack = ConnAck::default();
        connack.server_keep_alive = Some(128);
        test_property(
            connack,
            &mut dest,
            EXPECTED_LEN,
            EXPECTED_PROP_LEN,
            PropertyType::KeepAlive,
        );
        assert_eq!(0x80, dest[7]);
    }

    #[test]
    fn test_response_info() {
        let response_info = "topic/client/596892ea-5524-4b32-a624-3d9a322a6b52".to_string();
        let expected_prop_len = response_info.len() as u32 + 3;
        let expected_len = EXPECTED_MIN_CONNACK_LEN as u32 + expected_prop_len;
        let mut dest = BytesMut::new();
        let mut connack = ConnAck::default();
        connack.response_info = Some(response_info.clone());
        test_property(
            connack,
            &mut dest,
            expected_len,
            expected_prop_len,
            PropertyType::RespInfo,
        );
        assert_eq!(response_info.len() as u8, dest[7]);
    }

    #[test]
    fn test_server_ref() {
        let server_ref = "another.server.com:1883".to_string();
        let expected_prop_len = server_ref.len() as u32 + 3;
        let expected_len = EXPECTED_MIN_CONNACK_LEN as u32 + expected_prop_len;
        let mut dest = BytesMut::new();
        let mut connack = ConnAck::default();
        connack.server_ref = Some(server_ref.clone());
        test_property(
            connack,
            &mut dest,
            expected_len,
            expected_prop_len,
            PropertyType::ServerRef,
        );
        assert_eq!(server_ref.len() as u8, dest[7]);
    }

    #[test]
    fn test_auth_method() {
        let auth_method = "Authentication Method String".to_string();
        let expected_prop_len = auth_method.len() as u32 + 3;
        let expected_len = EXPECTED_MIN_CONNACK_LEN as u32 + expected_prop_len;
        let mut dest = BytesMut::new();
        let mut connack = ConnAck::default();
        connack.auth_method = Some(auth_method.clone());
        test_property(
            connack,
            &mut dest,
            expected_len,
            expected_prop_len,
            PropertyType::AuthMethod,
        );
        assert_eq!(auth_method.len() as u8, dest[7]);
    }

    #[test]
    fn test_auth_data() {
        const EXPECTED_LEN: u32 = EXPECTED_MIN_CONNACK_LEN as u32 + 43;
        const EXPECTED_PROP_LEN: u32 = 43;
        let mut dest = BytesMut::new();
        let mut connack = ConnAck::default();
        let auth_data = vec![0_u8; 40];
        connack.auth_data = Some(auth_data);
        test_property(
            connack,
            &mut dest,
            EXPECTED_LEN,
            EXPECTED_PROP_LEN,
            PropertyType::AuthData,
        );
    }

    fn test_bool_property(connack: ConnAck, property: PropertyType, expected: bool) {
        const EXPECTED_LEN: u32 = EXPECTED_MIN_CONNACK_LEN as u32 + PROP_SIZE_U8;
        const EXPECTED_PROP_LEN: u32 = PROP_SIZE_U8;
        let mut dest = BytesMut::new();
        test_property(
            connack,
            &mut dest,
            EXPECTED_LEN,
            EXPECTED_PROP_LEN,
            property,
        );
        assert_eq!(expected as u8, dest[6]);
    }

    fn test_property(
        connack: ConnAck,
        dest: &mut BytesMut,
        expected_len: u32,
        expected_prop_len: u32,
        property: PropertyType,
    ) {
        let result = connack.encode(dest);
        assert!(result.is_ok());
        assert_eq!(expected_len, dest.len() as u32);
        assert_eq!(expected_prop_len, connack.property_remaining().unwrap());
        assert_eq!(expected_prop_len as u8, dest[4]);
        assert_eq!(property as u8, dest[5]);
    }
}
