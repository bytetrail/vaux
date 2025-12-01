use crate::{
    codec::variable_byte_int_size,
    property::{PacketProperties, PropertyBundle},
    Decode, Encode, FixedHeader, HeaderSize, PacketType, PropertyType, Reason, Size,
};
use bytes::{Buf, BufMut};
use std::{collections::HashSet, sync::LazyLock};

const DEFAULT_DISCONNECT_REMAINING: u32 = 1;
static DISCONNECT_PROPS: LazyLock<HashSet<PropertyType>> = LazyLock::new(|| {
    let mut set = HashSet::new();
    set.insert(PropertyType::SessionExpiryInterval);
    set.insert(PropertyType::ReasonString);
    set.insert(PropertyType::UserProperty);
    set.insert(PropertyType::ServerReference);
    set
});

#[derive(Clone, Debug, PartialEq, Eq, vaux_macro::PacketProperties)]
pub struct Disconnect {
    pub reason: Reason,
    props: PropertyBundle,
}

impl Default for Disconnect {
    fn default() -> Self {
        Disconnect {
            reason: Reason::Success,
            props: PropertyBundle::new(&DISCONNECT_PROPS),
        }
    }
}

impl Disconnect {
    pub fn new(reason: Reason) -> Self {
        Self {
            reason,
            props: PropertyBundle::new(&DISCONNECT_PROPS),
        }
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
        self.props.size()
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
        dest.put_u8(self.reason as u8);
        self.props.encode(dest)?;
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
        self.props.decode(src)?;
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
        disconnect
            .properties_mut()
            .set_property(crate::property::Property::ReasonString(
                "failed".to_string(),
            ));
        let mut dest = BytesMut::new();
        match disconnect.encode(&mut dest) {
            Ok(_) => {
                assert_eq!("failed".len() + 7 as usize, dest.len());
            }
            Err(e) => panic!("Unexpected encoding error {:?}", e.to_string()),
        }
    }

    #[test]
    fn test_encode_server_ref() {
        const SERVER_REF: &'static str = "bytetrail.org";
        const PROP_LEN: u8 = 16;
        let mut disconnect = Disconnect::new(Reason::ServerMoved);
        disconnect
            .properties_mut()
            .set_property(crate::property::Property::ServerReference(
                SERVER_REF.to_string(),
            ));
        let mut dest = BytesMut::new();
        match disconnect.encode(&mut dest) {
            Ok(_) => {
                assert_eq!(PROP_LEN as usize + 4, dest.len());
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
