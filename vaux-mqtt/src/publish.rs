use crate::{
    codec::{self},
    fixed::FixedHeader,
    property::{PayloadFormat, UserProperty},
    CodecSize, Decode, Encode, MqttCodecError, PacketType, PropertyCodecSize, PropertyType,
    QoSLevel,
};
use vaux_macro::{CodecSize, Decode, Encode, PropertyCodecSize};

#[derive(Default, Debug, Clone, Eq, PartialEq, Encode, Decode, PropertyCodecSize, CodecSize)]
pub struct PublishHeader {
    pub topic_name: Option<String>,
    pub packet_id: Option<u16>,
    #[property(property_type = "PropertyType::PayloadFormat")]
    pub payload_format: Option<PayloadFormat>,
    #[property(property_type = "PropertyType::MessageExpiry")]
    pub message_expiry: Option<u32>,
    #[property(property_type = "PropertyType::TopicAlias")]
    pub topic_alias: Option<u16>,
    #[property(property_type = "PropertyType::ResponseTopic")]
    pub response_topic: Option<String>,
    #[property(property_type = "PropertyType::CorrelationData")]
    #[codec(skip_if(empty))]
    pub correlation_data: Vec<u8>,
    #[property(property_type = "PropertyType::SubscriptionIdentifier")]
    #[codec(
        encode_with("codec::encode_var_u32"),
        decode_with("codec::decode_var_u32")
    )]
    pub subscription_identifiers: u32,
    #[property(property_type = "PropertyType::ContentType")]
    pub content_type: Option<String>,
    #[property(property_type = "PropertyType::UserProperty")]
    pub user_properties: UserProperty,
}

pub type Publish = crate::packet::ControlPacket<PublishHeader, Vec<u8>>;

impl Publish {
    /// Create a new Publish packet with the given topic name and QoS level and message
    /// string payload. The payload format is set to UTF-8.
    ///
    /// ### Header Flags
    /// * QoS level is set to the given value.
    /// * Duplicate flag is set to false.
    /// * Retain flag is set to false.
    ///
    /// ### Errors
    /// * If `packet_id` is `None` and `qos_level` is not `AtMostOnce`, an error is returned.
    ///   This is because a packet identifier is required for QoS 1 and 2. See MQTTv5 specification
    ///   section 3.3.2.2.
    /// * If `packet_id` is `Some`, not 0, and `qos_level` is `AtMostOnce`, an error is returned.
    ///   This is because a packet identifier is not supported for QoS 0. See MQTTv5 specification
    ///  section 3.3.2.2.
    pub fn new_with_message(
        packet_id: Option<u16>,
        topic: String,
        qos_level: QoSLevel,
        message: &str,
    ) -> Result<Self, MqttCodecError> {
        Publish::new_with_payload(packet_id, topic, qos_level, message.as_bytes().to_vec())
            .and_then(|p| Ok(p.with_payload_format(PayloadFormat::Utf8)))
    }

    /// Create a new Publish packet with the given topic name and QoS level and
    /// binary payload.
    ///
    /// ### Header Flags
    /// * QoS level is set to the given value.
    /// * Duplicate flag is set to false.
    /// * Retain flag is set to false.
    ///
    /// ### Errors
    /// * If `packet_id` is `None` and `qos_level` is not `AtMostOnce`, an error is returned.
    ///   This is because a packet identifier is required for QoS 1 and 2. See MQTTv5 specification
    ///   section 3.3.2.2.
    /// * If `packet_id` is `Some`, not 0, and `qos_level` is `AtMostOnce`, an error is returned.
    ///   This is because a packet identifier is not supported for QoS 0. See MQTTv5 specification
    ///  section 3.3.2.2.
    pub fn new_with_payload(
        packet_id: Option<u16>,
        topic: String,
        qos_level: QoSLevel,
        payload: Vec<u8>,
    ) -> Result<Self, MqttCodecError> {
        if packet_id.is_none() && qos_level != QoSLevel::AtMostOnce {
            return Err(MqttCodecError::new(
                "Mqttv5 3.3.2.2 QOS level must be \"At Most Once\" (0) when no packet identifier set",
            ));
        } else if packet_id.is_some() && qos_level == QoSLevel::AtMostOnce {
            return Err(MqttCodecError::new(
                "Mqttv5 3.3.2.2 Packet Identifier not supported when QOS level is set to \"At Most Once\" (0)",
            ));
        }

        Publish {
            fixed_header: FixedHeader::new(PacketType::Publish),
            variable_header: PublishHeader {
                topic_name: Some(topic),
                packet_id,
                ..Default::default()
            },
            payload: payload,
        }
        .with_qos(qos_level)
        .with_payload_format(PayloadFormat::Bin)
        .with_dup(false)
    }

    pub fn new_from_header(fixed_header: FixedHeader) -> Result<Self, MqttCodecError> {
        match fixed_header.packet_type {
            PacketType::Publish => Ok(Publish {
                fixed_header,
                variable_header: PublishHeader::default(),
                payload: Vec::new(),
            }),
            p => Err(MqttCodecError {
                reason: format!("unable to construct from {p}"),
                kind: crate::codec::ErrorKind::MalformedPacket,
            }),
        }
    }

    pub fn topic_name(&self) -> Option<String> {
        self.variable_header.topic_name.clone()
    }

    pub fn set_topic_name(&mut self, topic: String) {
        self.variable_header.topic_name = Some(topic);
    }

    pub fn qos(&self) -> QoSLevel {
        self.fixed_header.qos()
    }

    pub fn set_qos(&mut self, qos: QoSLevel) {
        self.fixed_header.set_qos(qos);
    }

    pub fn with_qos(mut self, qos: QoSLevel) -> Self {
        self.set_qos(qos);
        self
    }

    pub fn dup(&self) -> bool {
        self.fixed_header.dup()
    }

    pub fn set_dup(&mut self, dup: bool) -> Result<(), MqttCodecError> {
        if self.fixed_header.qos() == QoSLevel::AtMostOnce && dup {
            return Err(MqttCodecError::new(
                "Mqttv53.3.2.2 QOS level must not be At
    Most Once with DUP true",
            ));
        }
        self.fixed_header.set_dup(dup);
        Ok(())
    }

    pub fn with_dup(mut self, dup: bool) -> Result<Self, MqttCodecError> {
        self.set_dup(dup)?;
        Ok(self)
    }

    pub fn retain(&self) -> bool {
        self.fixed_header.retain()
    }

    pub fn set_retain(&mut self, retain: bool) {
        self.fixed_header.set_retain(retain);
    }

    pub fn with_retain(mut self, retain: bool) -> Self {
        self.fixed_header.set_retain(retain);
        self
    }

    pub fn packet_id(&self) -> Option<u16> {
        self.variable_header.packet_id
    }

    pub fn set_packet_id(&mut self, id: Option<u16>) -> Result<(), MqttCodecError> {
        if self.fixed_header.qos() == QoSLevel::AtMostOnce {
            return Err(MqttCodecError::new(
                "Mqttv53.3.2.2 QOS level must not be At Most Once",
            ));
        }
        self.variable_header.packet_id = id;
        Ok(())
    }

    pub fn with_packet_id(mut self, id: Option<u16>) -> Result<Self, MqttCodecError> {
        self.set_packet_id(id)?;
        Ok(self)
    }

    pub fn with_topic_alias(mut self, alias: u16) -> Self {
        self.variable_header.topic_alias = Some(alias);
        self
    }

    pub fn payload_format(&self) -> Option<PayloadFormat> {
        self.variable_header.payload_format
    }

    pub fn set_payload_format(&mut self, format: PayloadFormat) {
        if format == PayloadFormat::Bin {
            self.variable_header.payload_format = None;
            return;
        }
        self.variable_header.payload_format = Some(format);
    }

    pub fn with_payload_format(mut self, format: PayloadFormat) -> Self {
        self.set_payload_format(format);
        self
    }

    pub fn message_expiry(&self) -> Option<u32> {
        self.variable_header.message_expiry
    }

    pub fn set_message_expiry(&mut self, expiry: u32) {
        self.variable_header.message_expiry = Some(expiry);
    }
}

#[cfg(test)]
mod test {
    use bytes::BytesMut;

    use super::*;

    const LEN_TOPIC_NAME_LEN: u32 = 2;
    const LEN_TOPIC_NAME: u32 = 5;

    #[test]
    fn test_fail_topic() {
        let hdr = crate::FixedHeader::new(PacketType::Publish);
        match Publish::new_from_header(hdr) {
            Ok(mut publish) => {
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
            Ok(mut publish) => {
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
    fn test_new_with_message() {
        let publish = Publish::new_with_message(
            Some(10),
            "topic".to_string(),
            QoSLevel::AtLeastOnce,
            "hello",
        )
        .expect("unable to create publish");
        assert_eq!(Some("topic".to_string()), publish.topic_name());
        assert_eq!(Some(10), publish.packet_id());
        assert_eq!(Some(PayloadFormat::Utf8), publish.payload_format());
        assert_eq!(b"hello".to_vec(), publish.payload);
    }

    #[test]
    fn test_new_with_payload() {
        let publish = Publish::new_with_payload(
            Some(10),
            "topic".to_string(),
            QoSLevel::AtLeastOnce,
            b"hello".to_vec(),
        )
        .expect("unable to create publish");
        assert_eq!(Some("topic".to_string()), publish.topic_name());
        assert_eq!(Some(10), publish.packet_id());
        assert_eq!(None, publish.payload_format());
        assert_eq!(b"hello".to_vec(), publish.payload);
    }

    #[test]
    fn test_new_with_payload_no_packet_id() {
        match Publish::new_with_payload(
            None,
            "topic".to_string(),
            QoSLevel::AtLeastOnce,
            b"hello".to_vec(),
        ) {
            Ok(_) => panic!("expected error"),
            Err(e) => assert!(e.reason.starts_with("Mqttv5 3.3.2.2")),
        }
    }
    #[test]
    fn test_new_with_message_no_packet_id() {
        match Publish::new_with_message(None, "topic".to_string(), QoSLevel::AtLeastOnce, "hello") {
            Ok(_) => panic!("expected error"),
            Err(e) => assert!(e.reason.starts_with("Mqttv5 3.3.2.2")),
        }
    }
}
