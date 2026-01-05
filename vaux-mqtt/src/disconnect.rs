use crate::{
    codec, property::UserProperty, Decode, Encode, FixedHeader, MqttCodecError, PacketType,
    PropertyCodecSize, PropertyType, Reason,
};
use vaux_macro::{CodecSize, Decode, Encode, PropertyCodecSize};

#[derive(Clone, Debug, PartialEq, Eq, CodecSize, PropertyCodecSize, Encode, Decode)]
pub struct DisconnectHeader {
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

impl Default for DisconnectHeader {
    fn default() -> Self {
        DisconnectHeader {
            reason: Reason::Success,
            session_expiry_interval: None,
            reason_string: None,
            server_reference: None,
            user_properties: crate::property::UserProperty::new(),
        }
    }
}

impl DisconnectHeader {
    pub fn new(reason: Reason) -> Self {
        Self {
            reason,
            ..Default::default()
        }
    }
}

pub type Disconnect = crate::packet::ControlPacket<DisconnectHeader, crate::packet::Empty>;

impl Disconnect {
    pub fn new_disconnect(reason: Reason) -> Self {
        let fixed_header = FixedHeader::new(PacketType::Disconnect);
        Disconnect {
            fixed_header,
            variable_header: DisconnectHeader::new(reason),
            payload: crate::packet::Empty {},
        }
    }

    pub fn reason(&self) -> Reason {
        self.variable_header.reason.clone()
    }

    pub fn set_reason(&mut self, reason: Reason) {
        self.variable_header.reason = reason;
    }
}
