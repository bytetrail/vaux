use crate::{
    codec, property::UserProperty, CodecSize, Decode, Encode, MqttCodecError, PropertyCodecSize,
    PropertyType,
};
use bytes::{Buf, BufMut};
use vaux_macro::{CodecSize, Decode, Encode, PropertyCodecSize};

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

impl Encode for PubAckRecReason {
    fn encode(&mut self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
        dest.put_u8(*self as u8);
        Ok(())
    }
}

impl Decode for PubAckRecReason {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
        let byte = src.get_u8();
        *self = PubAckRecReason::try_from(byte)?;
        Ok(1)
    }
}

impl CodecSize for PubAckRecReason {
    #[inline]
    fn codec_size(&self) -> u32 {
        1
    }
}

impl PropertyCodecSize for PubAckRecReason {
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

impl Encode for PubRelCompReason {
    fn encode(&mut self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
        dest.put_u8(*self as u8);
        Ok(())
    }
}

impl Decode for PubRelCompReason {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<u32, MqttCodecError> {
        let byte = src.get_u8();
        *self = PubRelCompReason::try_from(byte)?;
        Ok(1)
    }
}

impl CodecSize for PubRelCompReason {
    #[inline]
    fn codec_size(&self) -> u32 {
        1
    }
}

impl PropertyCodecSize for PubRelCompReason {
    #[inline]
    fn property_size(&self) -> u32 {
        2
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Encode, Decode, PropertyCodecSize, CodecSize)]
pub struct PubAckRec {
    pub packet_id: u16,
    reason: Option<PubAckRecReason>,
    #[codec(property_type = "PropertyType::ReasonString")]
    reason_desc: Option<String>,
    #[codec(property_type = "PropertyType::UserProperty")]
    user_properties: UserProperty,
}

pub type PubAck = PubAckRec;
pub type PubRec = PubAckRec;

#[derive(Default, Debug, Clone, PartialEq, Eq, Encode, Decode, PropertyCodecSize, CodecSize)]
pub struct PubRelComp {
    pub packet_id: u16,
    reason: Option<PubRelCompReason>,
    #[codec(property_type = "PropertyType::ReasonString")]
    reason_desc: Option<String>,
    #[codec(property_type = "PropertyType::UserProperty")]
    user_properties: UserProperty,
}

pub type PubRel = PubRelComp;
pub type PubComp = PubRelComp;

impl PubAckRec {
    pub fn new_with_packet_id(packet_id: u16) -> Self {
        PubAckRec {
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

impl PubRelComp {
    pub fn new_with_packet_id(packet_id: u16) -> Self {
        PubRelComp {
            packet_id,
            ..Default::default()
        }
    }
}
