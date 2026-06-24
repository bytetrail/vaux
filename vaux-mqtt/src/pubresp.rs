use crate::{
    codec::{self, ErrorKind},
    property::UserProperty,
    MqttCodecError, PacketType, PropertyType, Reason,
};
use vaux_macro::{CodecSize, Decode, Encode, PropertyCodecSize, PropertyEncode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PubRelCompReason {
    #[default]
    Success = 0x00,
    PacketIdInUse = 0x91,
}

pub type PubAck = PubResp;
pub type PubRec = PubResp;
pub type PubComp = PubResp;
pub type PubRel = PubResp;

#[derive(Default, Debug, Clone, PartialEq, Eq, PropertyCodecSize, PropertyEncode, Encode, Decode)]
#[codec(
    as_packet,
    abbreviated_when = "self.reason.unwrap_or_default() == codec::Reason::Success && self.property_size() == 0"
)]
#[derive(CodecSize)]
pub struct PubResp {
    #[codec(skip)]
    fixed_header: codec::FixedHeader,
    pub packet_id: u16,
    #[codec(non_abbreviated)]
    reason: Option<codec::Reason>,
    #[codec(property_type = "PropertyType::ReasonString")]
    reason_desc: Option<String>,
    #[codec(property_type = "PropertyType::UserProperty")]
    user_properties: UserProperty,
}

impl PubResp {
    pub fn new_puback_with_packet_id(packet_id: u16) -> Self {
        PubResp {
            fixed_header: codec::FixedHeader::new(codec::PacketType::PubAck),
            packet_id,
            ..Default::default()
        }
    }

    pub fn new_pubrec_with_packet_id(packet_id: u16) -> Self {
        PubResp {
            fixed_header: codec::FixedHeader::new(codec::PacketType::PubRec),
            packet_id,
            ..Default::default()
        }
    }

    pub fn new_pubrel_with_packet_id(packet_id: u16) -> Self {
        PubResp {
            fixed_header: codec::FixedHeader::new(codec::PacketType::PubRel),
            packet_id,
            ..Default::default()
        }
    }

    pub fn new_pubcomp_with_packet_id(packet_id: u16) -> Self {
        PubResp {
            fixed_header: codec::FixedHeader::new(codec::PacketType::PubComp),
            packet_id,
            ..Default::default()
        }
    }

    pub fn new_with_fixed_header(fixed_header: codec::FixedHeader) -> Result<Self, MqttCodecError> {
        match fixed_header.packet_type {
            codec::PacketType::PubAck
            | codec::PacketType::PubRec
            | codec::PacketType::PubComp
            | codec::PacketType::PubRel => Ok(PubResp {
                fixed_header,
                ..Default::default()
            }),
            _ => Err(MqttCodecError::new_with_kind(
                "Packet must be one of [PubAck, PubRec, PubComp, PubRel]",
                codec::ErrorKind::InvalidPacket,
            )),
        }
    }

    pub fn reason(&self) -> Option<Reason> {
        self.reason
    }

    pub fn set_reason(&mut self, reason: Reason) -> Result<(), MqttCodecError> {
        if self.supported_reason(reason) {
            self.reason = Some(reason);
            Ok(())
        } else {
            Err(MqttCodecError {
                reason: "unsupported reason".to_string(),
                kind: ErrorKind::UnsupportedReason(reason as u8),
            })
        }
    }

    fn supported_reason(&self, reason: Reason) -> bool {
        match self.fixed_header.packet_type {
            PacketType::PubAck | PacketType::PubRec => matches!(
                reason,
                Reason::Success
                    | Reason::NoSubscribers
                    | Reason::UnspecifiedErr
                    | Reason::ImplementationErr
                    | Reason::NotAuthorized
                    | Reason::InvalidTopicName
                    | Reason::PacketIdInUse
                    | Reason::QuotaExceeded
                    | Reason::PayloadFormatErr
            ),
            PacketType::PubComp | PacketType::PubRel => {
                matches!(reason, Reason::Success | Reason::PacketIdInUse)
            }
            _ => false,
        }
    }
}
