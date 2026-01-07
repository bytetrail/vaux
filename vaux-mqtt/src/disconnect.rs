use crate::{codec, property::UserProperty, MqttCodecError, PropertyType, Reason};
use vaux_macro::{packet, CodecSize, Decode, Encode, PropertyCodecSize};

#[packet(packet_type = "codec::PacketType::Disconnect")]
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Disconnect {
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

impl Disconnect {
    pub fn new(reason: Reason) -> Self {
        Self {
            reason,
            ..Default::default()
        }
    }
}
