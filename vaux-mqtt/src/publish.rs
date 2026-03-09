use crate::{codec, property::UserProperty, MqttCodecError, PacketType, PropertyType, QoSLevel};
use bytes::{Buf, BufMut, BytesMut};
use vaux_macro::packet;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PayloadFormat {
    #[default]
    Bin = 0x00,
    Utf8 = 0x01,
}

impl TryFrom<u8> for PayloadFormat {
    type Error = MqttCodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(PayloadFormat::Bin),
            0x01 => Ok(PayloadFormat::Utf8),
            _ => Err(MqttCodecError::new("invalid payload format")),
        }
    }
}

impl codec::Encode for PayloadFormat {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        dest.put_u8(*self as u8);
        Ok(())
    }
}

impl codec::Decode for PayloadFormat {
    fn decode(&mut self, src: &mut BytesMut) -> Result<u32, MqttCodecError> {
        *self = PayloadFormat::try_from(src.get_u8())?;
        Ok(1)
    }
}

impl codec::CodecSize for PayloadFormat {
    fn codec_size(&self) -> u32 {
        1
    }
}

#[packet(packet_type = "codec::PacketType::Publish")]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Publish {
    pub topic_name: String,
    packet_id: Option<u16>,
    #[codec(property_type = "PropertyType::PayloadFormat")]
    pub payload_format: Option<PayloadFormat>,
    #[codec(property_type = "PropertyType::MessageExpiry")]
    pub message_expiry: Option<u32>,
    #[codec(property_type = "PropertyType::TopicAlias")]
    pub topic_alias: Option<u16>,
    #[codec(property_type = "PropertyType::ResponseTopic")]
    pub response_topic: Option<String>,
    #[codec(
        property_type = "PropertyType::CorrelationData",
        skip_if = "Vec::is_empty"
    )]
    pub correlation_data: Vec<u8>,
    #[codec(property_type = "PropertyType::SubscriptionIdentifier")]
    #[codec(
        size_with = "codec::codec_size_opt_variable_byte_int_ref",
        encode_with = "codec::encode_opt_variable_byte_int_ref",
        decode_with = "codec::decode_opt_variable_byte_int"
    )]
    pub subscription_identifiers: Option<u32>,
    #[codec(property_type = "PropertyType::ContentType")]
    pub content_type: Option<String>,
    #[codec(property_type = "PropertyType::UserProperty")]
    pub user_properties: UserProperty,
    #[codec(
        payload_type = "remaining",
        encode_with = "codec::encode_opt_vec_u8_raw_ref",
        decode_with = "codec::decode_opt_vec_u8_raw",
        size_with = "codec::codec_size_opt_vec_u8_raw"
    )]
    pub payload: Option<Vec<u8>>,
}

impl Default for Publish {
    fn default() -> Self {
        Publish {
            fixed_header: codec::FixedHeader::new(PacketType::Publish),
            topic_name: String::new(),
            packet_id: None,
            payload_format: None,
            message_expiry: None,
            topic_alias: None,
            response_topic: None,
            correlation_data: Vec::new(),
            subscription_identifiers: None,
            content_type: None,
            user_properties: UserProperty::default(),
            payload: None,
        }
    }
}

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
            fixed_header: codec::FixedHeader::new(PacketType::Publish),
            topic_name: topic,
            packet_id,
            payload: Some(payload),
            ..Default::default()
        }
        .with_qos(qos_level)
        .with_payload_format(PayloadFormat::Bin)
        .with_dup(false)
    }

    pub fn new_from_header(fixed_header: codec::FixedHeader) -> Result<Self, MqttCodecError> {
        match fixed_header.packet_type {
            PacketType::Publish => Ok(Publish {
                fixed_header,
                ..Default::default()
            }),
            p => Err(MqttCodecError {
                reason: format!("unable to construct from {p}"),
                kind: crate::codec::ErrorKind::MalformedPacket,
            }),
        }
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
        self.packet_id
    }

    pub fn set_packet_id(&mut self, id: Option<u16>) -> Result<(), MqttCodecError> {
        if self.fixed_header.qos() == QoSLevel::AtMostOnce {
            return Err(MqttCodecError::new(
                "Mqttv53.3.2.2 QOS level must not be At Most Once",
            ));
        }
        self.packet_id = id;
        Ok(())
    }

    pub fn with_packet_id(mut self, id: Option<u16>) -> Result<Self, MqttCodecError> {
        self.set_packet_id(id)?;
        Ok(self)
    }

    pub fn with_topic_alias(mut self, alias: u16) -> Self {
        self.topic_alias = Some(alias);
        self
    }

    pub fn set_payload_format(&mut self, format: PayloadFormat) {
        if format == PayloadFormat::Bin {
            self.payload_format = None;
            return;
        }
        self.payload_format = Some(format);
    }

    pub fn with_payload_format(mut self, format: PayloadFormat) -> Self {
        self.set_payload_format(format);
        self
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_new_with_message() {
        let publish = Publish::new_with_message(
            Some(10),
            "topic".to_string(),
            QoSLevel::AtLeastOnce,
            "hello",
        )
        .expect("unable to create publish");
        assert_eq!("topic".to_string(), publish.topic_name);
        assert_eq!(Some(10), publish.packet_id());
        assert_eq!(Some(PayloadFormat::Utf8), publish.payload_format);
        assert_eq!(Some(b"hello".to_vec()), publish.payload);
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
        assert_eq!("topic".to_string(), publish.topic_name);
        assert_eq!(Some(10), publish.packet_id());
        assert_eq!(None, publish.payload_format);
        assert_eq!(Some(b"hello".to_vec()), publish.payload);
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
