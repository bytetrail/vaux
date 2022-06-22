use bytes::BufMut;

use crate::{
    codec::{encode_utf8_string, variable_byte_int_size, PropertyType, PROP_SIZE_U32},
    Encode, FixedHeader, PacketType, Reason, Remaining, UserPropertyMap,
};

const DEFAULT_DISCONNECT_REMAINING: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disconnect {
    pub reason: Reason,
    pub session_expiry: Option<u32>,
    pub reason_desc: Option<String>,
    pub user_properties: Option<UserPropertyMap>,
    pub server_ref: Option<String>,
}

impl Disconnect {
    pub fn new(reason: Reason) -> Self {
        Disconnect {
            reason,
            session_expiry: None,
            reason_desc: None,
            user_properties: None,
            server_ref: None,
        }
    }
}

impl Remaining for Disconnect {
    fn size(&self) -> u32 {
        let property_remaining = self.property_remaining().unwrap();
        let len = variable_byte_int_size(property_remaining);
        1 + len + property_remaining
    }

    fn property_remaining(&self) -> Option<u32> {
        let remaining =
            1 + if self.session_expiry.is_some() {
                PROP_SIZE_U32
            } else {
                0
            } + self
                .reason_desc
                .as_ref()
                .map_or(0, |s| (s.len() + 3) as u32)
                + if self.user_properties.is_some() {
                    self.user_properties.as_ref().unwrap().size()
                } else {
                    0
                }
                + self.server_ref.as_ref().map_or(0, |s| (s.len() + 3) as u32);
        Some(remaining)
    }

    fn payload_remaining(&self) -> Option<u32> {
        None
    }
}

impl Encode for Disconnect {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), crate::MQTTCodecError> {
        let mut header = FixedHeader::new(PacketType::Disconnect);
        let prop_remaining = self.property_remaining().unwrap();
        header.set_remaining(
            DEFAULT_DISCONNECT_REMAINING + prop_remaining + variable_byte_int_size(prop_remaining),
        );
        header.encode(dest)?;
        if let Some(expiry) = self.session_expiry {
            dest.put_u8(PropertyType::SessionExpiry as u8);
            dest.put_u32(expiry);
        }
        if let Some(reason) = &self.reason_desc {
            dest.put_u8(PropertyType::Reason as u8);
            encode_utf8_string(reason, dest)?;
        }
        if let Some(properties) = &self.user_properties {
            properties.encode(dest)?;
        }
        if let Some(reference) = &self.server_ref {
            dest.put_u8(PropertyType::ServerRef as u8);
            encode_utf8_string(reference, dest)?;
        }

        Ok(())
    }
}
