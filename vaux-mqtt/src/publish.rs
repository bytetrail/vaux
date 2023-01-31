use std::collections::HashSet;

use bytes::{Buf, BufMut, BytesMut};

use crate::{
    codec::{
        check_property, decode_binary_data, decode_utf8_string, decode_variable_len_integer,
        encode_bin_property, encode_bool_property, encode_u16_property, encode_u32_property,
        encode_utf8_property, encode_utf8_string, encode_var_int_property,
        encode_variable_len_integer, variable_byte_int_size, PROP_SIZE_BINARY, PROP_SIZE_U16,
        PROP_SIZE_U32, PROP_SIZE_U8, PROP_SIZE_UTF8_STRING, SIZE_UTF8_STRING,
    },
    Decode, Encode, FixedHeader, MQTTCodecError, PacketType, PropertyType, QoSLevel, Size,
    UserPropertyMap,
};

const RETAIN_MASK: u8 = 0b_0000_0001;
const DUP_MASK: u8 = 0b_0000_1000;

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct Publish {
    pub dup: bool,
    pub qos: QoSLevel,
    pub retain: bool,
    pub topic_name: Option<String>,
    pub topic_alias: Option<u16>,
    pub response_topic: Option<String>,
    pub packet_id: Option<u16>,
    pub payload_utf8: bool,
    pub message_expiry: Option<u32>,
    pub correlation_data: Option<Vec<u8>>,
    pub user_props: Option<UserPropertyMap>,
    pub sub_id: Option<Vec<u32>>,
    pub content_type: Option<String>,
    pub payload: Option<Vec<u8>>,
}

impl Publish {
    pub fn new_from_header(hdr: FixedHeader) -> Result<Self, MQTTCodecError> {
        let qos = QoSLevel::try_from(hdr.flags)?;
        let retain = hdr.flags & RETAIN_MASK != 0;
        let dup = hdr.flags & DUP_MASK != 0;
        if dup && qos == QoSLevel::AtMostOnce {
            return Err(MQTTCodecError::new(
                "[MQTT 3.1.1.1] DUP must be 0 for QOS level \"At most once\"",
            ));
        }
        Ok(Publish {
            dup,
            qos,
            retain,
            topic_name: None,
            topic_alias: None,
            response_topic: None,
            packet_id: None,
            payload_utf8: false,
            message_expiry: None,
            correlation_data: None,
            user_props: None,
            sub_id: None,
            content_type: None,
            payload: None,
        })
    }

    pub fn set_packet_id(&mut self, id: u16) -> Result<(), MQTTCodecError> {
        if self.qos == QoSLevel::AtMostOnce {
            return Err(MQTTCodecError::new(
                "MQTTv53.3.2.2 QOS level must not be At Most Once",
            ));
        }
        self.packet_id = Some(id);
        Ok(())
    }

    fn decode_property(
        &mut self,
        property_type: PropertyType,
        src: &mut BytesMut,
    ) -> Result<(), MQTTCodecError> {
        match property_type {
            PropertyType::PayloadFormat => self.payload_utf8 = src.get_u8() == 1,
            PropertyType::MessageExpiry => self.message_expiry = Some(src.get_u32()),
            PropertyType::TopicAlias => self.topic_alias = Some(src.get_u16()),
            PropertyType::ResponseTopic => self.response_topic = Some(decode_utf8_string(src)?),
            PropertyType::CorrelationData => self.correlation_data = Some(decode_binary_data(src)?),
            PropertyType::SubscriptionId => {
                if self.sub_id.is_none() {
                    self.sub_id = Some(Vec::new());
                }
                self.sub_id
                    .as_mut()
                    .unwrap()
                    .push(decode_variable_len_integer(src));
            }
            PropertyType::ContentType => self.content_type = Some(decode_utf8_string(src)?),
            prop => {
                return Err(MQTTCodecError::new(&format!(
                    "MQTTv5 3.3.2.3 unexpected property type value: {}",
                    prop
                )))
            }
        }
        Ok(())
    }
}

impl Size for Publish {
    fn size(&self) -> u32 {
        let mut remaining = if let Some(topic_name) = self.topic_name.as_ref() {
            SIZE_UTF8_STRING + topic_name.len() as u32
        } else {
            SIZE_UTF8_STRING
        };
        // packet identifier
        remaining += if let QoSLevel::AtMostOnce = self.qos {
            0
        } else {
            2
        };
        let payload_size = self.payload_size();
        let prop_remaining = self.property_size();
        remaining + payload_size + prop_remaining + variable_byte_int_size(prop_remaining)
    }

    fn property_size(&self) -> u32 {
        let mut remaining = if self.payload_utf8 { PROP_SIZE_U8 } else { 0 };
        remaining += self.message_expiry.map_or(0, |_| PROP_SIZE_U32)
            + self.topic_alias.map_or(0, |_| PROP_SIZE_U16)
            + self
                .response_topic
                .as_ref()
                .map_or(0, |t| PROP_SIZE_UTF8_STRING + t.len() as u32)
            + self
                .correlation_data
                .as_ref()
                .map_or(0, |c| PROP_SIZE_BINARY + c.len() as u32)
            + self.user_props.as_ref().map_or(0, |p| p.size())
            + self
                .content_type
                .as_ref()
                .map_or(0, |c| PROP_SIZE_UTF8_STRING + c.len() as u32);
        // handle multiple subscription identifiers
        if let Some(v) = self.sub_id.as_ref() {
            for id in v {
                remaining += variable_byte_int_size(*id) + 1;
            }
        }
        return remaining;
    }

    fn payload_size(&self) -> u32 {
        if let Some(payload) = &self.payload {
            payload.len() as u32
        } else {
            0
        }
    }
}

impl Encode for Publish {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MQTTCodecError> {
        let mut header = FixedHeader::new(PacketType::Publish);
        let size = self.size();
        let property_remaining = self.property_size();
        header.set_remaining(size);
        header.encode(dest)?;
        if self.topic_name.is_none() && self.topic_alias.is_none() {
            return Err(MQTTCodecError::new(
                "MQTTv5 3.3.2.1 must have topic name or topic alias",
            ));
        }
        match self.topic_name.as_ref() {
            Some(topic_name) => encode_utf8_string(topic_name, dest)?,
            None => encode_utf8_string("", dest)?,
        };
        if self.qos != QoSLevel::AtMostOnce {
            if let Some(packet_id) = self.packet_id {
                dest.put_u16(packet_id);
            } else {
                return Err(MQTTCodecError::new(
                    "MQTTv5 3.3.2.2 packet identifier must be included for QOS 1 or 2",
                ));
            }
        }
        encode_variable_len_integer(property_remaining, dest);
        if self.payload_utf8 {
            encode_bool_property(PropertyType::PayloadFormat, self.payload_utf8, dest);
        }
        if let Some(expiry) = self.message_expiry {
            encode_u32_property(PropertyType::MessageExpiry, expiry, dest);
        }
        if let Some(alias) = self.topic_alias {
            encode_u16_property(PropertyType::TopicAlias, alias, dest);
        }
        if let Some(r) = self.response_topic.as_ref() {
            encode_utf8_property(PropertyType::ResponseTopic, r, dest)?;
        }
        if let Some(c) = self.correlation_data.as_ref() {
            encode_bin_property(PropertyType::CorrelationData, c, dest)?;
        }
        if let Some(user_props) = self.user_props.as_ref() {
            user_props.encode(dest)?;
        }
        if let Some(s) = self.sub_id.as_ref() {
            for id in s {
                encode_var_int_property(PropertyType::SubscriptionId, *id, dest);
            }
        }
        if let Some(c) = self.content_type.as_ref() {
            encode_utf8_property(PropertyType::ContentType, c, dest)?;
        }
        if let Some(p) = self.payload.as_ref() {
            dest.put_slice(p)
        }
        Ok(())
    }
}

impl Decode for Publish {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<(), MQTTCodecError> {
        let topic_name = decode_utf8_string(src)?;
        if topic_name.len() == 0 {
            self.topic_name = None;
        } else {
            self.topic_name = Some(topic_name);
        }
        if self.qos != QoSLevel::AtMostOnce {
            self.packet_id = Some(src.get_u16());
        }
        let property_len = decode_variable_len_integer(src);
        if property_len > src.remaining() as u32 {
            return Err(MQTTCodecError::new(
                "MQTTv5 2.2.2.1 property length exceeds packet size",
            ));
        }
        if property_len == 1 {
            return Err(MQTTCodecError::new(
                "MQTTv5 2.2.2.1 invalid property length",
            ));
        }
        let payload_len = src.remaining() - property_len as usize;
        let mut properties: HashSet<PropertyType> = HashSet::new();
        while src.remaining() > payload_len {
            match PropertyType::try_from(src.get_u8()) {
                Ok(property_type) => {
                    if property_type != PropertyType::UserProperty {
                        check_property(property_type, &mut properties)?;
                        self.decode_property(property_type, src)?;
                    } else {
                        if self.user_props == None {
                            self.user_props = Some(UserPropertyMap::new());
                        }
                        let property_map = self.user_props.as_mut().unwrap();
                        let key = decode_utf8_string(src)?;
                        let value = decode_utf8_string(src)?;
                        property_map.add_property(&key, &value);
                    }
                }
                Err(err) => return Err(err),
            };
        }
        if src.remaining() > 0 {
            match src.get(src.len() - src.remaining()..src.remaining()) {
                Some(p) => self.payload = Some(Vec::from(p)),
                None => return Err(MQTTCodecError::new("unable to decode payload")),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const LEN_TOPIC_NAME_LEN: u32 = 2;
    const LEN_TOPIC_NAME: u32 = 5;

    #[test]
    fn test_encode_no_props() {
        const LEN_PROP_LEN: u32 = 1;
        const EXPECTED_REMAINING: u32 = LEN_TOPIC_NAME_LEN + LEN_TOPIC_NAME + LEN_PROP_LEN;
        const EXPECTED_LEN: u32 = EXPECTED_REMAINING + 2;
        let hdr = crate::FixedHeader {
            packet_type: crate::PacketType::Publish,
            flags: 0b_0000_0000,
            remaining: 0,
        };
        match Publish::new_from_header(hdr) {
            Ok(mut publish) => {
                publish.topic_name = Some("topic".to_string());
                let mut dest = BytesMut::new();
                match publish.encode(&mut dest) {
                    Ok(_) => {
                        assert_eq!(
                            EXPECTED_LEN,
                            dest.len() as u32,
                            "expected length to be {}",
                            EXPECTED_LEN
                        );
                        assert_eq!(EXPECTED_REMAINING, dest[1] as u32);
                        assert_eq!(LEN_TOPIC_NAME, dest[3] as u32);
                        assert_eq!(
                            "topic".to_string(),
                            String::from_utf8(Vec::from(dest.get(4..9).unwrap())).unwrap()
                        );
                    }
                    Err(e) => panic!("unable to encode publish record: {}", e),
                }
            }
            Err(e) => panic!("unable to create publish record: {}", e),
        }
    }

    #[test]
    fn test_fail_topic() {
        let hdr = crate::FixedHeader {
            packet_type: crate::PacketType::Publish,
            flags: 0b_0000_0000,
            remaining: 0,
        };
        match Publish::new_from_header(hdr) {
            Ok(publish) => {
                let mut dest = BytesMut::new();
                match publish.encode(&mut dest) {
                    Ok(_) => panic!("expected error on encode"),
                    Err(e) => {
                        assert_eq!("MQTTv5 3.3.2.1", &e.reason[0..14]);
                    }
                }
            }
            Err(e) => panic!("unable to encode publish record: {}", e),
        }
    }

    #[test]
    fn test_fail_packet_id() {
        let hdr = crate::FixedHeader {
            packet_type: crate::PacketType::Publish,
            flags: 0b_0000_0000,
            remaining: 0,
        };
        match Publish::new_from_header(hdr) {
            Ok(publish) => {
                let mut dest = BytesMut::new();
                match publish.encode(&mut dest) {
                    Ok(_) => panic!("expected error on encode"),
                    Err(e) => {
                        assert_eq!("MQTTv5 3.3.2.1", &e.reason[0..14]);
                    }
                }
            }
            Err(e) => panic!("unable to encode publish record: {}", e),
        }
    }

    #[test]
    fn test_basic_payload() {
        const LEN_PROP_LEN: u32 = 1;
        const LEN_PAYLOAD: u32 = 20;
        const EXPECTED_REMAINING: u32 =
            LEN_TOPIC_NAME_LEN + LEN_TOPIC_NAME + LEN_PROP_LEN + LEN_PAYLOAD;
        const EXPECTED_LEN: u32 = EXPECTED_REMAINING + 2;
        let hdr = crate::FixedHeader {
            packet_type: crate::PacketType::Publish,
            flags: 0b_0000_0000,
            remaining: 0,
        };
        match Publish::new_from_header(hdr) {
            Ok(mut publish) => {
                let mut dest = BytesMut::new();
                publish.payload = Some([10_u8; 20].into());
                publish.topic_name = Some(String::from("topic"));
                match publish.encode(&mut dest) {
                    Ok(_) => {
                        assert_eq!(
                            EXPECTED_LEN,
                            dest.len() as u32,
                            "expected length to be {}",
                            EXPECTED_LEN
                        );
                        assert_eq!(EXPECTED_REMAINING, dest[1] as u32);
                        let payload =
                            dest.get((EXPECTED_LEN - LEN_PAYLOAD) as usize..EXPECTED_LEN as usize);
                        for v in payload.unwrap() {
                            assert_eq!(10_u8, *v, "expected 10 for payload byte value");
                        }
                    }
                    Err(e) => panic!("unexpected error on encode {}", e),
                }
            }
            Err(e) => panic!("unable to encode publish record: {}", e),
        }
    }

    #[test]
    fn test_decode_payload() {
        const TEST_PACKET: [u8; 12] = [
            0x00, 0x04, 0x76, 0x61, 0x75, 0x78, 0x00, 0x68, 0x65, 0x6c, 0x6c, 0x6f,
        ];
        let mut src = BytesMut::from(&TEST_PACKET[..]);
        let mut hdr = FixedHeader::new(PacketType::Publish);
        hdr.remaining = 0x0c;
        let mut publish_packet = Publish::new_from_header(hdr).expect("unable to create packet");
        match publish_packet.decode(&mut src) {
            Ok(_) => {
                assert_eq!("vaux", publish_packet.topic_name.unwrap());
                assert_eq!(
                    "hello",
                    String::from_utf8(publish_packet.payload.unwrap()).expect("unable to decode")
                );
            }
            Err(e) => panic!("unexpected error decoding publish: {}", e),
        }
    }

    #[test]
    /// MQTTv5
    fn test_decode_packet_id() {
        const EXPECTED_PACKET_ID: u16 = 270;
        const TEST_PACKET_ID: [u8; 9] = [0x00, 0x04, 0x76, 0x61, 0x75, 0x78, 0x01, 0x0e, 0x00];
        let mut src = BytesMut::from(&TEST_PACKET_ID[..]);
        let mut hdr = FixedHeader::new(PacketType::Publish);
        hdr.flags = 0x01;
        hdr.remaining = 0x09;
        let mut publish_packet =
            Publish::new_from_header(hdr.clone()).expect("unable to create packet");
        match publish_packet.decode(&mut src) {
            Ok(_) => {
                assert_eq!(EXPECTED_PACKET_ID, publish_packet.packet_id.unwrap());
                assert_eq!("vaux", publish_packet.topic_name.unwrap());
            }
            Err(e) => panic!("unexpected error decoding publish: {}", e),
        }
        hdr.flags = 0x00;
        let mut src = BytesMut::from(&TEST_PACKET_ID[..]);
        let mut publish_packet = Publish::new_from_header(hdr).expect("unable to create packet");
        match publish_packet.decode(&mut src) {
            Ok(_) => {
                panic!("expected malformed packet");
            }
            Err(e) => {
                assert_eq!("MQTTv5 2.2.2.1 invalid property length", e.reason);
            }
        }
    }
}
