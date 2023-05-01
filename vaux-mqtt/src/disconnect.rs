use std::collections::HashSet;

use bytes::{Buf, BufMut, BytesMut};

use crate::{
    codec::{
        check_property, get_utf8, get_var_u32, put_utf8, put_var_u32, variable_byte_int_size,
        PROP_SIZE_U32, PROP_SIZE_UTF8_STRING,
    },
    Decode, Encode, FixedHeader, MqttCodecError, PacketType, PropertyType, Reason, Size,
};

const DEFAULT_DISCONNECT_REMAINING: u32 = 1;

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Disconnect {
    pub reason: Reason,
    pub session_expiry: Option<u32>,
    pub reason_desc: Option<String>,
    pub server_ref: Option<String>,
}

impl Disconnect {
    pub fn new(reason: Reason) -> Self {
        Self {
            reason,
            session_expiry: None,
            reason_desc: None,
            server_ref: None,
        }
    }

    fn decode_properties(&mut self, src: &mut BytesMut) -> Result<(), MqttCodecError> {
        let prop_size = get_var_u32(src);
        let read_until = src.remaining() - prop_size as usize;
        let mut properties: HashSet<PropertyType> = HashSet::new();
        while src.remaining() > read_until {
            match PropertyType::try_from(src.get_u8()) {
                Ok(property_type) => {
                    if property_type != PropertyType::UserProperty {
                        check_property(property_type, &mut properties)?;
                        match property_type {
                            PropertyType::SessionExpiryInt => {
                                self.session_expiry = Some(src.get_u32())
                            }
                            PropertyType::ReasonString => self.reason_desc = Some(get_utf8(src)?),
                            PropertyType::ServerRef => self.server_ref = Some(get_utf8(src)?),
                            val => {
                                return Err(MqttCodecError::new(&format!(
                                    "unexpected property type: {}",
                                    val
                                )))
                            }
                        }
                    } else {
                        if self.user_props.is_none() {
                            self.user_props = Some(UserPropertyMap::default());
                        }
                        let property_map = self.user_props.as_mut().unwrap();
                        let key = get_utf8(src)?;
                        let value = get_utf8(src)?;
                        property_map.add_property(&key, &value);
                    }
                }
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }
}

impl Size for Disconnect {
    fn size(&self) -> u32 {
        let remaining = self.property_size();
        if remaining == 0 && self.reason == Reason::Success {
            0
        } else {
            let len = variable_byte_int_size(remaining);
            DEFAULT_DISCONNECT_REMAINING + len + remaining
        }
    }

    fn property_size(&self) -> u32 {
        let mut remaining: u32 = 0;
        if self.session_expiry.is_some() {
            remaining += PROP_SIZE_U32;
        }
        remaining += self
            .reason_desc
            .as_ref()
            .map_or(0, |r| PROP_SIZE_UTF8_STRING + r.len() as u32);
        remaining += self.user_props.as_ref().map_or(0, |p| p.size());
        remaining += self
            .server_ref
            .as_ref()
            .map_or(0, |r| PROP_SIZE_UTF8_STRING + r.len() as u32);
        remaining
    }

    /// The Disconnect packet does not have a payload. None is returned
    fn payload_size(&self) -> u32 {
        0
    }
}

impl Encode for Disconnect {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), crate::MqttCodecError> {
        let mut header = FixedHeader::new(PacketType::Disconnect);
        let prop_remaining = self.property_size();
        header.remaining = DEFAULT_DISCONNECT_REMAINING + variable_byte_int_size(prop_remaining);
        if self.reason == Reason::Success && prop_remaining == 0 {
            header.remaining = 0;
            header.encode(dest)?;
            return Ok(());
        }
        header.encode(dest)?;
        let reason = self.reason as u8;
        dest.put_u8(reason);
        let prop_size = self.property_size();
        if prop_size > 0 {
            put_var_u32(prop_size, dest);
            if let Some(expiry) = self.session_expiry {
                dest.put_u32(expiry);
            }
            if let Some(reason_desc) = self.reason_desc.as_ref() {
                put_utf8(reason_desc, dest)?;
            }
            if let Some(user_props) = &self.user_props {
                user_props.encode(dest)?;
            }
            if let Some(server_ref) = self.server_ref.as_ref() {
                put_utf8(server_ref, dest)?;
            }
        }
        Ok(())
    }
}

impl Decode for Disconnect {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<(), crate::MqttCodecError> {
        // Mqtt v5 specification 3.14.2.1
        if src.remaining() == 0 {
            self.reason = Reason::Success;
            return Ok(());
        }
        self.reason = Reason::try_from(src.get_u8())?;
        if src.remaining() > 0 {
            let property_remaining = get_var_u32(src);
            if property_remaining > 0 {
                self.decode_properties(src)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use bytes::BytesMut;

    use super::*;

    #[test]
    fn test_no_remaining() {
        let disconnect = Disconnect::new(Reason::Success);
        let mut dest = BytesMut::new();
        match disconnect.encode(&mut dest) {
            Ok(_) => {
                assert_eq!(2 as usize, dest.len());
                assert_eq!(0, dest[1]);
            }
            Err(e) => panic!("Unexpected encoding error {:?}", e.to_string()),
        }
    }

    #[test]
    fn test_encode_reason_desc() {
        let mut disconnect = Disconnect::new(Reason::ImplementationErr);
        disconnect.reason_desc = Some("failed".to_string());
        let mut dest = BytesMut::new();
        match disconnect.encode(&mut dest) {
            Ok(_) => {
                assert_eq!("failed".len() + 6 as usize, dest.len());
            }
            Err(e) => panic!("Unexpected encoding error {:?}", e.to_string()),
        }
    }

    #[test]
    fn test_encode_server_ref() {
        const SERVER_REF: &'static str = "bytetrail.org";
        const PROP_LEN: u8 = 16;
        let mut disconnect = Disconnect::new(Reason::ServerMoved);
        disconnect.server_ref = Some(SERVER_REF.to_string());
        let mut dest = BytesMut::new();
        match disconnect.encode(&mut dest) {
            Ok(_) => {
                assert_eq!(SERVER_REF.len() + 6 as usize, dest.len());
                assert_eq!(Reason::ServerMoved as u8, dest[2]);
                assert_eq!(PROP_LEN, dest[3]);
            }
            Err(e) => panic!("Unexpected encoding error {:?}", e.to_string()),
        }
    }

    #[test]
    fn test_basic_decode() {
        let encoded: [u8; 0] = [];
        let mut src = BytesMut::new();
        src.extend_from_slice(&encoded);
        let mut disconnect = Disconnect::default();
        disconnect.reason = Reason::ImplementationErr;
        let result = disconnect.decode(&mut src);
        assert!(
            result.is_ok(),
            "Unexpected error decoding: {}",
            result.unwrap_err()
        );
        assert_eq!(Reason::Success, disconnect.reason);
    }

    #[test]
    fn test_decode_with_reason() {
        let encoded: [u8; 2] = [Reason::AdminAction as u8, 0x00];
        let mut src = BytesMut::new();
        src.extend_from_slice(&encoded);
        let mut disconnect = Disconnect::default();
        disconnect.reason = Reason::ImplementationErr;
        let result = disconnect.decode(&mut src);
        assert!(
            result.is_ok(),
            "Unexpected error decoding: {}",
            result.unwrap_err()
        );
        assert_eq!(Reason::AdminAction, disconnect.reason);
    }
}
