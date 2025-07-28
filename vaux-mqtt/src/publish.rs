use crate::{
    codec::{get_utf8, put_utf8, variable_byte_int_size, SIZE_UTF8_STRING},
    property::{PacketProperties, PayloadFormat, Property, PropertyBundle},
    Decode, Encode, FixedHeader, MqttCodecError, PacketType, PropertyType, QoSLevel, Size,
};
use bytes::{Buf, BufMut};
use std::collections::HashSet;

lazy_static! {
    static ref SUPPORTED: HashSet<PropertyType> = {
        let mut set = HashSet::new();
        set.insert(PropertyType::PayloadFormat);
        set.insert(PropertyType::MessageExpiry);
        set.insert(PropertyType::TopicAlias);
        set.insert(PropertyType::ResponseTopic);
        set.insert(PropertyType::CorrelationData);
        set.insert(PropertyType::SubscriptionIdentifier);
        set.insert(PropertyType::ContentType);
        set.insert(PropertyType::UserProperty);
        set
    };
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Publish {
    pub header: FixedHeader,
    pub topic_name: Option<String>,
    pub packet_id: Option<u16>,
    props: PropertyBundle,
    payload: Option<Vec<u8>>,
}

impl Default for Publish {
    fn default() -> Self {
        Self {
            header: FixedHeader::new(PacketType::Publish),
            topic_name: None,
            packet_id: None,
            props: PropertyBundle::new(SUPPORTED.clone()),
            payload: None,
        }
    }
}

impl Publish {
    pub fn new_from_header(header: FixedHeader) -> Result<Self, MqttCodecError> {
        match header.packet_type {
            PacketType::Publish => Ok(Publish {
                header,
                topic_name: None,
                packet_id: None,
                props: PropertyBundle::new(SUPPORTED.clone()),
                payload: None,
            }),
            p => Err(MqttCodecError {
                reason: format!("unable to construct from {p}"),
                kind: crate::codec::ErrorKind::MalformedPacket,
            }),
        }
    }

    pub fn qos(&self) -> QoSLevel {
        self.header.qos()
    }

    pub fn set_qos(&mut self, qos: QoSLevel) {
        self.header.set_qos(qos);
    }

    pub fn set_payload(&mut self, data: Vec<u8>) {
        self.payload = Some(data);
    }

    pub fn take_payload(&mut self) -> Option<Vec<u8>> {
        self.payload.take()
    }

    pub fn set_packet_id(&mut self, id: u16) -> Result<(), MqttCodecError> {
        if self.header.qos() == QoSLevel::AtMostOnce {
            return Err(MqttCodecError::new(
                "Mqttv53.3.2.2 QOS level must not be At Most Once",
            ));
        }
        self.packet_id = Some(id);
        Ok(())
    }

    pub fn topic_alias(&self) -> Option<u16> {
        self.props
            .get_property(PropertyType::TopicAlias)
            .and_then(|p| {
                if let Property::TopicAlias(alias) = p {
                    Some(*alias)
                } else {
                    None
                }
            })
    }

    pub fn set_topic_alias(&mut self, alias: u16) {
        self.props.set_property(Property::TopicAlias(alias))
    }

    pub fn payload_format(&self) -> Option<PayloadFormat> {
        self.props
            .get_property(PropertyType::PayloadFormat)
            .and_then(|p| {
                if let Property::PayloadFormat(format) = p {
                    Some(*format)
                } else {
                    None
                }
            })
    }

    pub fn set_payload_format(&mut self, format: PayloadFormat) {
        if format == PayloadFormat::Bin {
            self.props.clear_property(PropertyType::PayloadFormat);
            return;
        }
        self.props.set_property(Property::PayloadFormat(format));
    }
}

impl PacketProperties for Publish {
    fn properties(&self) -> &PropertyBundle {
        &self.props
    }
    fn properties_mut(&mut self) -> &mut PropertyBundle {
        &mut self.props
    }

    fn set_properties(&mut self, properties: PropertyBundle) {
        self.props = properties;
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
        remaining += if let QoSLevel::AtMostOnce = self.header.qos() {
            0
        } else {
            2
        };
        let payload_size = self.payload_size();
        let prop_remaining = self.property_size();
        remaining + payload_size + prop_remaining + variable_byte_int_size(prop_remaining)
    }

    fn property_size(&self) -> u32 {
        self.props.size()
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
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
        let mut header = self.header.clone();
        let size = self.size();
        header.set_remaining(size);
        header.encode(dest)?;
        if self.topic_name.is_none() && self.props.get_property(PropertyType::TopicAlias).is_none()
        {
            return Err(MqttCodecError::new(
                "MQTTv5 3.3.2.1 must have topic name or topic alias",
            ));
        }
        match self.topic_name.as_ref() {
            Some(topic_name) => put_utf8(topic_name, dest)?,
            None => put_utf8("", dest)?,
        };
        if self.header.qos() != QoSLevel::AtMostOnce {
            if let Some(packet_id) = self.packet_id {
                dest.put_u16(packet_id);
            } else {
                return Err(MqttCodecError::new(
                    "MQTTv5 3.3.2.2 packet identifier must be included for QOS 1 or 2",
                ));
            }
        }
        self.props.encode(dest)?;
        if let Some(p) = self.payload.as_ref() {
            dest.put_slice(p)
        }
        Ok(())
    }
}

impl Decode for Publish {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
        let topic_name = get_utf8(src)?;
        if topic_name.is_empty() {
            self.topic_name = None;
        } else {
            self.topic_name = Some(topic_name);
        }
        if self.header.qos() != QoSLevel::AtMostOnce {
            self.packet_id = Some(src.get_u16());
        }
        self.props.decode(src)?;
        if src.remaining() > 0 {
            let idx = src.len() - src.remaining();
            let end = src.remaining();
            self.payload = Some(src.copy_to_bytes(end - idx).to_vec());
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use bytes::BytesMut;

    use super::*;

    const LEN_TOPIC_NAME_LEN: u32 = 2;
    const LEN_TOPIC_NAME: u32 = 5;

    #[test]
    fn test_encode_no_props() {
        const LEN_PROP_LEN: u32 = 1;
        const EXPECTED_REMAINING: u32 = LEN_TOPIC_NAME_LEN + LEN_TOPIC_NAME + LEN_PROP_LEN;
        const EXPECTED_LEN: u32 = EXPECTED_REMAINING + 2;
        let hdr = crate::FixedHeader::new(PacketType::Publish);
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
        let hdr = crate::FixedHeader::new(PacketType::Publish);
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
        let hdr = crate::FixedHeader::new(PacketType::Publish);
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
        let hdr = crate::FixedHeader::new(PacketType::Publish);
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
        hdr.set_qos(QoSLevel::AtLeastOnce);
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
        hdr.clear_flags();
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
