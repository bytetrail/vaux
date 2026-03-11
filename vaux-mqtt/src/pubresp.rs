use crate::{
    codec::{self, CodecSize, Decode, ErrorKind, PropertyCodecSize},
    property::UserProperty,
    MqttCodecError, PacketType, PropertyType, Reason,
};
use bytes::{Buf, BufMut};
use vaux_macro::PropertyCodecSize;

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

#[derive(Default, Debug, Clone, PartialEq, Eq, PropertyCodecSize)]
pub struct PubResp {
    fixed_header: codec::FixedHeader,
    pub packet_id: u16,
    reason: Option<codec::Reason>,
    #[codec(property_type = "PropertyType::ReasonString")]
    reason_desc: Option<String>,
    #[codec(property_type = "PropertyType::UserProperty")]
    user_properties: UserProperty,
}

impl codec::CodecSize for PubResp {
    fn codec_size(&self) -> u32 {
        let mut size = 2; // packet_id size
        let property_size = self.property_size();

        if self.reason.unwrap_or_default() == codec::Reason::Success && self.property_size() == 0 {
            return size;
        }
        size += 1; // reason code size
        size += codec::variable_byte_int_size(property_size);
        if let Some(reason_desc) = &self.reason_desc {
            size += 3 + reason_desc.len() as u32; // property identifier + length + string bytes
        }
        size += self.user_properties.property_size();
        size
    }
}

impl codec::Encode for PubResp {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
        self.fixed_header.encode(dest)?;
        if self.reason.unwrap_or_default() == codec::Reason::Success && self.property_size() == 0 {
            // reason code and remaining packet is optional in this case
            dest.put_u8(2); // remaining length
            dest.put_u16(self.packet_id);
            return Ok(());
        }
        codec::encode_variable_byte_int(self.codec_size(), dest)?;
        dest.put_u16(self.packet_id);
        let reason = self.reason.unwrap_or_default();
        dest.put_u8(reason as u8);
        codec::encode_variable_byte_int(self.property_size(), dest)?;
        if let Some(v) = self.reason_desc.as_ref() {
            dest.put_u8(PropertyType::ReasonString as u8);
            codec::encode_string(v, dest)?;
        }
        self.user_properties.encode(dest)?;
        Ok(())
    }
}

impl Decode for PubResp {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<usize, MqttCodecError> {
        if src.remaining() < 2 {
            return Err(MqttCodecError::new_with_kind(
                "Insufficient data",
                codec::ErrorKind::InsufficientData(2, src.remaining()),
            ));
        }
        self.packet_id = src.get_u16();
        if src.remaining() == 0 {
            return Ok(2);
        }
        self.reason = Some(codec::Reason::try_from(src.get_u8())?);
        let property_length = codec::decode_variable_byte_int(src)?;
        let mut read_bytes = 3 + property_length.1;
        if src.remaining() < property_length.0 as usize {
            return Err(MqttCodecError::new_with_kind(
                "Insufficient data for properties",
                codec::ErrorKind::InsufficientData(property_length.0 as usize, src.remaining()),
            ));
        }
        let mut properties_bytes = src.split_to(property_length.0 as usize);
        while properties_bytes.has_remaining() {
            let property_type = PropertyType::try_from(properties_bytes.get_u8())?;
            match property_type {
                PropertyType::ReasonString => {
                    let (value, len) = codec::decode_string(&mut properties_bytes)?;
                    self.reason_desc = Some(value);
                    read_bytes += 1 + len;
                }
                PropertyType::UserProperty => {
                    read_bytes += self.user_properties.decode(&mut properties_bytes)?;
                }
                _ => {
                    return Err(MqttCodecError::new_with_kind(
                        "Invalid property for PUBREL/PUBCOMP",
                        codec::ErrorKind::InvalidPacket,
                    ));
                }
            }
        }
        Ok(read_bytes)
    }
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
