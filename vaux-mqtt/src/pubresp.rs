use std::collections::HashSet;

use bytes::{Buf, BufMut};

use crate::{
    codec::variable_byte_int_size, property::PropertyBundle, Decode, Encode, FixedHeader,
    MqttCodecError, PacketType, PropertyType, Reason, Size,
};

const VARIABLE_HEADER_LEN: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubResp {
    resp_type: PacketType,
    reason: Reason,
    pub packet_id: u16,
    props: PropertyBundle,
}

impl PubResp {
    pub fn new_puback() -> Self {
        PubResp::new(PacketType::PubAck).unwrap()
    }

    pub fn new_pubcomp() -> Self {
        PubResp::new(PacketType::PubComp).unwrap()
    }
    pub fn new_pubrec() -> Self {
        PubResp::new(PacketType::PubRec).unwrap()
    }
    pub fn new_pubrel() -> Self {
        PubResp::new(PacketType::PubRel).unwrap()
    }

    fn new(resp_type: PacketType) -> Result<Self, MqttCodecError> {
        let mut supported = HashSet::new();
        supported.insert(PropertyType::ReasonString);
        supported.insert(PropertyType::UserProperty);

        match resp_type {
            PacketType::PubAck | PacketType::PubComp | PacketType::PubRec | PacketType::PubRel => {
                Ok(Self {
                    resp_type,
                    reason: Reason::Success,
                    packet_id: 0,
                    props: PropertyBundle::new(supported),
                })
            }
            _ => Err(MqttCodecError {
                reason: "unsupported response type".to_string(),
            }),
        }
    }

    pub fn reason(&self) -> Reason {
        self.reason
    }

    pub fn set_reason(&mut self, reason: Reason) -> Result<(), MqttCodecError> {
        if PubResp::supported_reason(&self.resp_type, &reason) {
            self.reason = reason;
            Ok(())
        } else {
            Err(MqttCodecError {
                reason: "unsupported reason".to_string(),
            })
        }
    }

    pub fn properties(&self) -> &PropertyBundle {
        &self.props
    }

    pub fn properties_mut(&mut self) -> &mut PropertyBundle {
        &mut self.props
    }

    fn supported_reason(resp_type: &PacketType, reason: &Reason) -> bool {
        match resp_type {
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

impl Size for PubResp {
    fn size(&self) -> u32 {
        let prop_size = self.property_size();
        let prop_size_len = variable_byte_int_size(prop_size);

        if prop_size == 0 {
            VARIABLE_HEADER_LEN
        } else {
            let remaining = 3 + prop_size + prop_size_len;
            VARIABLE_HEADER_LEN + remaining
        }
    }

    fn property_size(&self) -> u32 {
        self.props.size()
    }

    fn payload_size(&self) -> u32 {
        0
    }
}

impl Encode for PubResp {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), crate::MqttCodecError> {
        let mut header = match self.resp_type {
            PacketType::PubAck => FixedHeader::new(PacketType::PubAck),
            PacketType::PubRec => FixedHeader::new(PacketType::PubRec),
            PacketType::PubComp => FixedHeader::new(PacketType::PubComp),
            PacketType::PubRel => FixedHeader::new(PacketType::PubRel),
            _ => {
                return Err(MqttCodecError {
                    reason: "usupported response type".to_string(),
                })
            }
        };
        header.set_remaining(self.size());
        header.encode(dest)?;
        dest.put_u16(self.packet_id);
        if self.reason == Reason::Success && self.props.is_empty() {
            Ok(())
        } else {
            dest.put_u8(self.reason as u8);
            self.props.encode(dest)?;
            Ok(())
        }
    }
}

impl Decode for PubResp {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<(), crate::MqttCodecError> {
        if src.remaining() == 2 {
            self.reason = Reason::Success;
            self.packet_id = src.get_u16();
            return Ok(());
        }
        self.packet_id = src.get_u16();
        self.reason = Reason::try_from(src.get_u8())?;
        if src.remaining() > 0 {
            self.props.decode(src)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use bytes::BytesMut;

    use crate::property::Property;

    use super::*;

    #[test]
    fn basic_encode() {
        const EXPECTED_LEN: usize = 4;
        let mut pubrec = PubResp::new_pubrec();
        pubrec.packet_id = 12345;
        assert!(pubrec.set_reason(Reason::Success).is_ok());
        let mut dest = BytesMut::new();
        let result = pubrec.encode(&mut dest);
        assert!(result.is_ok());
        assert_eq!(EXPECTED_LEN, dest.len());
    }

    #[test]
    fn encode_with_reason() {
        const EXPECTED_LEN: usize = 25;
        let mut pubrec = PubResp::new_pubrec();
        pubrec.packet_id = 12345;
        assert!(pubrec.set_reason(Reason::UnspecifiedErr).is_ok());
        pubrec
            .properties_mut()
            .set_property(Property::ReasonString("unable to comply".to_string()));

        let mut dest = BytesMut::new();
        let result = pubrec.encode(&mut dest);
        assert!(result.is_ok());
        assert_eq!(EXPECTED_LEN, dest.len());
    }
}
