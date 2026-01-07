use crate::{codec, property::UserProperty, MqttCodecError, PropertyType};
use bytes::{Buf, BufMut};
use vaux_macro::packet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PubAckRecReason {
    #[default]
    Success = 0x00,
    NoSubscribers = 0x10,
    UnspecifiedErr = 0x80,
    ImplementationErr = 0x83,
    NotAuthorized = 0x87,
    InvalidTopicName = 0x90,
    PacketIdInUse = 0x91,
    QuotaExceeded = 0x97,
    PayloadFormatErr = 0x99,
}

impl TryFrom<u8> for PubAckRecReason {
    type Error = MqttCodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(PubAckRecReason::Success),
            0x10 => Ok(PubAckRecReason::NoSubscribers),
            0x80 => Ok(PubAckRecReason::UnspecifiedErr),
            0x83 => Ok(PubAckRecReason::ImplementationErr),
            0x87 => Ok(PubAckRecReason::NotAuthorized),
            0x90 => Ok(PubAckRecReason::InvalidTopicName),
            0x91 => Ok(PubAckRecReason::PacketIdInUse),
            0x97 => Ok(PubAckRecReason::QuotaExceeded),
            0x99 => Ok(PubAckRecReason::PayloadFormatErr),
            _ => Err(MqttCodecError::new_with_kind(
                "Unsupported reason code",
                codec::ErrorKind::UnsupportedReason(value),
            )),
        }
    }
}

impl codec::Encode for PubAckRecReason {
    fn encode(&mut self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
        dest.put_u8(*self as u8);
        Ok(())
    }
}

impl codec::Decode for PubAckRecReason {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
        let byte = src.get_u8();
        *self = PubAckRecReason::try_from(byte)?;
        Ok(1)
    }
}

impl codec::CodecSize for PubAckRecReason {
    #[inline]
    fn codec_size(&self) -> u32 {
        1
    }
}

impl codec::PropertyCodecSize for PubAckRecReason {
    #[inline]
    fn property_size(&self) -> u32 {
        2
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PubRelCompReason {
    #[default]
    Success = 0x00,
    PacketIdInUse = 0x91,
}

impl TryFrom<u8> for PubRelCompReason {
    type Error = MqttCodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(PubRelCompReason::Success),
            0x91 => Ok(PubRelCompReason::PacketIdInUse),
            _ => Err(MqttCodecError::new_with_kind(
                "Unsupported reason code",
                codec::ErrorKind::UnsupportedReason(value),
            )),
        }
    }
}

impl codec::Encode for PubRelCompReason {
    fn encode(&mut self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
        dest.put_u8(*self as u8);
        Ok(())
    }
}

impl codec::Decode for PubRelCompReason {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
        let byte = src.get_u8();
        *self = PubRelCompReason::try_from(byte)?;
        Ok(1)
    }
}

impl codec::CodecSize for PubRelCompReason {
    #[inline]
    fn codec_size(&self) -> u32 {
        1
    }
}

impl codec::PropertyCodecSize for PubRelCompReason {
    #[inline]
    fn property_size(&self) -> u32 {
        2
    }
}

#[packet(packet_type = "codec::PacketType::PubAck")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct PubAck {
    pub packet_id: u16,
    reason: Option<PubAckRecReason>,
    #[codec(property_type = "PropertyType::ReasonString")]
    reason_desc: Option<String>,
    #[codec(property_type = "PropertyType::UserProperty")]
    user_properties: UserProperty,
}

#[packet(packet_type = "codec::PacketType::PubRec")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct PubRec {
    pub packet_id: u16,
    reason: Option<PubAckRecReason>,
    #[codec(property_type = "PropertyType::ReasonString")]
    reason_desc: Option<String>,
    #[codec(property_type = "PropertyType::UserProperty")]
    user_properties: UserProperty,
}

#[packet(packet_type = "codec::PacketType::PubRel")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct PubRel {
    pub packet_id: u16,
    reason: Option<PubRelCompReason>,
    #[codec(property_type = "PropertyType::ReasonString")]
    reason_desc: Option<String>,
    #[codec(property_type = "PropertyType::UserProperty")]
    user_properties: UserProperty,
}

#[packet(packet_type = "codec::PacketType::PubComp")]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct PubComp {
    pub packet_id: u16,
    reason: Option<PubRelCompReason>,
    #[codec(property_type = "PropertyType::ReasonString")]
    reason_desc: Option<String>,
    #[codec(property_type = "PropertyType::UserProperty")]
    user_properties: UserProperty,
}

impl PubAck {
    pub fn new_with_packet_id(packet_id: u16) -> Self {
        PubAck {
            packet_id,
            ..Default::default()
        }
    }
}

impl PubRec {
    pub fn new_with_packet_id(packet_id: u16) -> Self {
        PubRec {
            packet_id,
            ..Default::default()
        }
    }
}

pub enum PublishResponse {
    PubAck,
    PubRec,
    PubComp,
    PubRel,
}

impl PubRel {
    pub fn new_with_packet_id(packet_id: u16) -> Self {
        PubRel {
            packet_id,
            ..Default::default()
        }
    }
}

impl PubComp {
    pub fn new_with_packet_id(packet_id: u16) -> Self {
        PubComp {
            packet_id,
            ..Default::default()
        }
    }
}
