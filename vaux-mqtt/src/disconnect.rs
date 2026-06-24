use crate::{
    codec::{self},
    property::UserProperty,
    MqttCodecError, PropertyType, Reason,
};
use vaux_macro::{CodecSize, Decode, Encode, PropertyCodecSize, PropertyEncode};

#[derive(Clone, Debug, PartialEq, Eq, PropertyCodecSize, PropertyEncode, Encode, Decode)]
#[codec(
    as_packet,
    abbreviated_when = "self.reason == Reason::NormalDisconnect && self.property_size() == 0"
)]
#[derive(CodecSize)]
pub struct Disconnect {
    #[codec(skip)]
    fixed_header: codec::FixedHeader,
    #[codec(non_abbreviated)]
    pub reason: Reason,
    #[codec(property_type = "PropertyType::SessionExpiryInterval")]
    pub session_expiry_interval: Option<u32>,
    #[codec(property_type = "PropertyType::ReasonString")]
    pub reason_string: Option<String>,
    #[codec(property_type = "PropertyType::ServerReference")]
    pub server_reference: Option<String>,
    #[codec(property_type = "PropertyType::UserProperty")]
    pub user_properties: UserProperty,
}

impl Default for Disconnect {
    fn default() -> Self {
        Self {
            fixed_header: codec::FixedHeader::new(codec::PacketType::Disconnect),
            reason: Reason::NormalDisconnect,
            session_expiry_interval: None,
            reason_string: None,
            server_reference: None,
            user_properties: UserProperty::default(),
        }
    }
}

impl Disconnect {
    pub fn new(reason: Reason) -> Self {
        Self {
            fixed_header: codec::FixedHeader::new(codec::PacketType::Disconnect),
            reason,
            ..Default::default()
        }
    }

    pub fn new_with_fixed_header(
        fixed_header: codec::FixedHeader,
    ) -> Result<Self, codec::MqttCodecError> {
        Ok(Self {
            fixed_header,
            ..Default::default()
        })
    }
}
