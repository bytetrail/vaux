use crate::{
    codec, property::UserProperty, Decode, Encode, MqttCodecError, PropertyCodecSize, PropertyType,
    Reason,
};
use vaux_macro::{CodecSize, Decode, Encode, PropertyCodecSize};

#[derive(Clone, Debug, PartialEq, Eq, CodecSize, PropertyCodecSize, Encode, Decode)]
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

impl Default for Disconnect {
    fn default() -> Self {
        Disconnect {
            reason: Reason::Success,
            session_expiry_interval: None,
            reason_string: None,
            server_reference: None,
            user_properties: crate::property::UserProperty::new(),
        }
    }
}

impl Disconnect {
    pub fn new(reason: Reason) -> Self {
        Self {
            reason,
            ..Default::default()
        }
    }
}
