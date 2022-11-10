use std::{
    collections::{HashSet},
};

use bytes::{Buf, BufMut, BytesMut};

use crate::{
    codec::{
        check_property, decode_utf8_string, decode_variable_len_integer, encode_utf8_string,
        encode_variable_len_integer, variable_byte_int_size, PropertyType, PROP_SIZE_U32,
    },
    Decode, Encode, FixedHeader, MQTTCodecError, PacketType, Reason, Remaining, UserPropertyMap,
};

const DEFAULT_DISCONNECT_REMAINING: u32 = 1;

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Disconnect {
    reason: Reason,
    session_expiry: Option<u32>,
    reason_desc: Option<String>,
    server_ref: Option<String>,
    user_props: Option<UserPropertyMap>,
}

impl Disconnect {
    pub fn new(reason: Reason) -> Self {
        Self {
            reason,
            session_expiry: None,
            reason_desc: None,
            server_ref: None,
            user_props: None,
        }
    }

    fn decode_properties(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError> {
        let prop_size = decode_variable_len_integer(src);
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
                            PropertyType::Reason => {
                                self.reason_desc = Some(decode_utf8_string(src)?)
                            }
                            PropertyType::ServerRef => {
                                self.server_ref = Some(decode_utf8_string(src)?)
                            }
                            val => {
                                return Err(MQTTCodecError::new(&format!(
                                    "unexpected property type: {}",
                                    val
                                )))
                            }
                        }
                    } else {
                        if self.user_props == None {
                            self.user_props = Some(UserPropertyMap::new());
                        }
                        let property_map = self.user_props.as_mut().unwrap();
                        let key = decode_utf8_string(src)?;
                        let value = decode_utf8_string(src)?;
                        property_map.add_property(&key, &value);
                    }
                }
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }
}

impl Remaining for Disconnect {
    fn size(&self) -> u32 {
        let remaining = self.property_remaining();
        if remaining.is_none() && self.reason == Reason::Success {
            0
        } else {
            let len = variable_byte_int_size(remaining.unwrap());
            DEFAULT_DISCONNECT_REMAINING + len + remaining.unwrap()
        }
    }

    fn property_remaining(&self) -> Option<u32> {
        let mut remaining: u32 = 0;
        if self.session_expiry.is_some() {
            remaining += PROP_SIZE_U32;
        }
        remaining += self.reason_desc.as_ref().map_or(0, |r| 3 + r.len() as u32);
        if let Some(props) = self.user_props.as_ref() {
            remaining += props.size();
        }
        self.server_ref.as_ref().map_or(0, |r| 2 + r.len());
        Some(remaining)
    }

    /// The Disconnect packet does not have a payload. None is returned
    fn payload_remaining(&self) -> Option<u32> {
        None
    }
}

impl Encode for Disconnect {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), crate::MQTTCodecError> {
        let mut header = FixedHeader::new(PacketType::Disconnect);
        let prop_remaining = self.property_remaining().unwrap();
        header.remaining = DEFAULT_DISCONNECT_REMAINING
            + variable_byte_int_size(prop_remaining)
            + DEFAULT_DISCONNECT_REMAINING;
        if self.reason == Reason::Success && prop_remaining == 0 {
            header.remaining = 0;
            header.encode(dest)?;
            return Ok(());
        }
        header.encode(dest)?;
        let reason = self.reason as u8;
        dest.put_u8(reason);
        if let Some(reason_desc) = self.reason_desc.as_ref() {
            encode_utf8_string(reason_desc, dest)?;
        }
        if let Some(user_props) = &self.user_props {
            user_props.encode(dest)?;
        }
        if let Some(server_ref) = self.server_ref.as_ref() {
            encode_utf8_string(server_ref, dest)?;
        }
        Ok(())
    }
}

impl Decode for Disconnect {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<(), crate::MQTTCodecError> {
        let len = decode_variable_len_integer(src);
        // MQTT v5 specification 3.14.2.1
        if len == 0 {
            self.reason = Reason::Success;
            return Ok(());
        }
        self.reason = Reason::try_from(src.get_u8())?;
        self.decode_properties(src)?;
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
                assert_eq!("failed".len() + 5 as usize, dest.len());
            }
            Err(e) => panic!("Unexpected encoding error {:?}", e.to_string()),
        }
    }

    #[test]
    fn test_encode_server_ref() {
        const SERVER_REF: &'static str = "bytetrail.org";
        let mut disconnect = Disconnect::new(Reason::ServerMoved);
        disconnect.server_ref = Some(SERVER_REF.to_string());
        let mut dest = BytesMut::new();
        match disconnect.encode(&mut dest) {
            Ok(_) => {
                assert_eq!(SERVER_REF.len() + 5 as usize, dest.len());
            }
            Err(e) => panic!("Unexpected encoding error {:?}", e.to_string()),
        }
    }

    #[test]
    fn test_basic_decode() {
        let encoded: [u8; 1] = [
            0x00,
        ];
        let mut src = BytesMut::new();
        src.extend_from_slice(&encoded);
        let mut disconnect = Disconnect::default();
        disconnect.reason = Reason::ImplementationErr;
        let result = disconnect.decode(&mut src);
        assert!(result.is_ok(), "Unexpected error decoding: {}", result.unwrap_err());
        assert_eq!(Reason::Success, disconnect.reason);
    }

    #[test]
    fn test_decode_with_reason() {
        let encoded: [u8; 3] = [
            0x01,
            Reason::AdminAction as u8,
            0x00,
        ];
        let mut src = BytesMut::new();
        src.extend_from_slice(&encoded);
        let mut disconnect = Disconnect::default();
        disconnect.reason = Reason::ImplementationErr;
        let result = disconnect.decode(&mut src);
        assert!(result.is_ok(), "Unexpected error decoding: {}", result.unwrap_err());
        assert_eq!(Reason::AdminAction, disconnect.reason);

    }

}
