use crate::codec::{ErrorKind, MAX_VARIABLE_BYTE_INT, MIN_VARIABLE_BYTE_INT};
use crate::{codec, MqttCodecError, PropertyType};
use crate::{property::UserProperty, MqttError, MqttVersion, QoSLevel};
use bytes::{Buf, BufMut};
use vaux_macro::{packet, CodecSize, Decode, Encode};

/// MQTT v5 3.8.3.1 Subscription Options
/// bits 4 and 5 of the subscription options hold the retain handling flag.
/// Retain handling is used to determine how messages published with the
/// retain flag set to ```true``` are handled when the ```SUBSCRIBE``` packet
/// is received.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum RetainHandling {
    #[default]
    Send,
    SendNew,
    None,
}

impl TryFrom<u8> for RetainHandling {
    type Error = MqttCodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(RetainHandling::Send),
            0x01 => Ok(RetainHandling::SendNew),
            0x02 => Ok(RetainHandling::None),
            v => Err(MqttCodecError::new(
                format!("Mqttv5 3.8.3.1 invalid retain option: {v}").as_str(),
            )),
        }
    }
}

#[packet(packet_type = "codec::PacketType::SubAck")]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SubAck {
    packet_id: u16,
    #[codec(property_type = "PropertyType::ReasonString")]
    pub reason: Option<String>,
    #[codec(property_type = "PropertyType::UserProperty")]
    pub user_properties: UserProperty,
    #[codec(
        payload_type = "remaining",
        size_with = "codec::codec_size_vec_u8_raw",
        encode_with = "encode_suback_reason_vec",
        decode_with = "decode_suback_reason_vec"
    )]
    pub reason_codes: Vec<u8>,
}

pub fn encode_suback_reason_vec(
    reasons: &Vec<u8>,
    dest: &mut bytes::BytesMut,
) -> Result<(), MqttCodecError> {
    dest.put_slice(reasons);
    Ok(())
}

pub fn decode_suback_reason_vec(
    reasons: &mut Vec<u8>,
    src: &mut bytes::BytesMut,
) -> Result<u32, MqttCodecError> {
    let len = src.remaining();
    reasons.extend_from_slice(&src[..]);
    src.advance(len);
    Ok(len as u32)
}

impl SubAck {
    pub fn new_with_packet_id(packet_id: u16) -> Result<Self, MqttCodecError> {
        if packet_id == 0 {
            return Err(MqttCodecError::new_with_kind("2.2.1 Packet identified must not be 0", ErrorKind::InvalidPacketIdentifier));    
        }
        Ok(Self {
            packet_id,
            ..Default::default()
        })  
    }

    pub fn packet_id(&self) -> u16 {
        self.packet_id
    }

    pub fn set_packet_id(&mut self, packet_id: u16) -> Result<(), MqttCodecError> {
        if packet_id == 0 {
            return Err(MqttCodecError::new_with_kind("2.2.1 Packet identified must not be 0", ErrorKind::InvalidPacketIdentifier));    
        }
        self.packet_id = packet_id;
        Ok(())
    }
}

/// Subscription represents an MQTT v5 3.8.3 SUBSCRIBE Payload. The Mqtt v5
/// 3.8.3.1 options are represented as individual fields in the struct.
#[derive(Debug, Default, Clone, PartialEq, Eq, Encode, Decode, CodecSize)]
pub struct SubscriptionFilter {
    pub filter: String,
    options: u8,
}

impl SubscriptionFilter {
    /// Create a new Subscription with the given filter and QoSLevel.
    /// The remaining fields are set to their default values.
    pub fn new(filter: String, qos: QoSLevel) -> Self {
        let mut s = Self {
            filter,
            options: 0_u8,
        };
        s.set_qos(qos);
        s
    }

    pub fn qos(&self) -> QoSLevel {
        (self.options & 0b0000_0011)
            .try_into()
            .unwrap_or(QoSLevel::AtMostOnce)
    }

    pub fn with_qos(mut self, qos: QoSLevel) -> Self {
        self.set_qos(qos);
        self
    }

    pub fn set_qos(&mut self, qos: QoSLevel) {
        self.options = (self.options & !0b0000_0011) | (qos as u8 & 0b0000_0011);
    }

    pub fn no_local(&self) -> bool {
        (self.options & 0b0000_0100) != 0
    }

    pub fn set_no_local(&mut self, no_local: bool) {
        self.options = (self.options & !0b0000_0100) | ((no_local as u8) << 2);
    }

    pub fn retain_handling(&self) -> RetainHandling {
        (self.options & 0b0000_1000)
            .try_into()
            .unwrap_or(RetainHandling::Send)
    }

    pub fn set_retain_handling(&mut self, retain: RetainHandling) {
        self.options = (self.options & !0b0000_1000) | ((retain as u8) << 3);
    }
}

#[packet(packet_type = "codec::PacketType::Subscribe")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Subscribe {
    pub packet_id: u16,
    #[codec(property_type = "PropertyType::SubscriptionIdentifier")]
    #[codec(
        size_with = "codec::codec_size_opt_variable_byte_int_ref",
        encode_with = "codec::encode_opt_variable_byte_int_ref",
        decode_with = "codec::decode_opt_variable_byte_int"
    )]
    pub subscription_id: Option<u32>,
    #[codec(property_type = "PropertyType::UserProperty")]
    pub props: UserProperty,
    #[codec(payload_type = "remaining")]
    pub filter: Vec<SubscriptionFilter>,
}

impl Subscribe {
    pub fn new_with_packet_id(packet_id: u16) -> Self {
        Self {
            packet_id,
            ..Default::default()
        }
    }

    pub fn new_with_filter(packet_id: u16, filter: Vec<SubscriptionFilter>) -> Self {
        Self {
            packet_id,
            filter,
            ..Default::default()
        }
    }

    pub fn set_subscription_id(&mut self, id: u32) -> Result<(), MqttError> {
        if !(MIN_VARIABLE_BYTE_INT..=MAX_VARIABLE_BYTE_INT).contains(&id) {
            return Err(MqttError::new_from_spec(
                MqttVersion::V5,
                "3.8.3",
                "subscription ID must be between 1 and 268,435,455",
            ));
        }
        self.subscription_id = Some(id);
        Ok(())
    }

    pub fn add_filter(&mut self, filter: SubscriptionFilter) {
        self.filter.push(filter);
    }

    pub fn remove_filter_at(&mut self, index: usize) -> Option<SubscriptionFilter> {
        if index < self.filter.len() {
            Some(self.filter.remove(index))
        } else {
            None
        }
    }

    pub fn remove_filter_with_topic(&mut self, topic: &str) -> Option<SubscriptionFilter> {
        if let Some(pos) = self.filter.iter().position(|f| f.filter == topic) {
            Some(self.filter.remove(pos))
        } else {
            None
        }
    }

    pub fn clear_filters(&mut self) {
        self.filter.clear();
    }
}
