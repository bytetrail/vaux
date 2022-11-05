use std::io::Read;

use bytes::{BufMut, Buf};

use crate::{codec::{variable_byte_int_size, PROP_SIZE_U32, encode_variable_len_integer, encode_utf8_string}, Decode, Encode, Reason, Remaining, UserPropertyMap, FixedHeader, PacketType};

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
        header.remaining = DEFAULT_DISCONNECT_REMAINING + variable_byte_int_size(prop_remaining) + DEFAULT_DISCONNECT_REMAINING;
        if self.reason == Reason::Success && prop_remaining == 0 {
            header.remaining = 0;
            header.encode(dest)?;
            return Ok(());
        }
        header.encode(dest)?;
        let reason = self.reason as u8;
        dest.put_u8(reason);
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
        let len = src.get_u16();
        // MQTT v5 specification 3.14.2.1
        if len == 0 {
            self.reason = Reason::Success;
            return Ok(());
        }
        self.reason = Reason::try_from(src.get_u8())?;
        Ok(())
    }
}


#[cfg(test)]
mod test {

    #[test]
    fn test_no_remaining() {
        
    }
}