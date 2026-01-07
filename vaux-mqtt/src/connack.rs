use crate::codec::{self, CodecSize, PropertyCodecSize, Reason};
use crate::property::{PropertyType, UserProperty};
use crate::{MqttCodecError, QoSLevel};
use vaux_macro::packet;

const CONNACK_SESSION_PRESENT_MASK: u8 = 0x01;

#[packet(packet_type = "codec::PacketType::ConnAck")]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConnAck {
    ack_flags: u8,
    pub reason: Reason,
    #[codec(property_type = "PropertyType::SessionExpiryInterval")]
    pub session_expiry_interval: Option<u32>,
    #[codec(property_type = "PropertyType::RecvMax")]
    pub receive_maximum: Option<u16>,
    #[codec(property_type = "PropertyType::MaxQoS")]
    pub maximum_qos: Option<QoSLevel>,
    #[codec(property_type = "PropertyType::RetainAvail")]
    pub retain_available: Option<bool>,
    #[codec(property_type = "PropertyType::MaxPacketSize")]
    pub maximum_packet_size: Option<u32>,
    #[codec(property_type = "PropertyType::AssignedClientId")]
    pub assigned_client_id: Option<String>,
    #[codec(property_type = "PropertyType::TopicAliasMax")]
    pub topic_alias_maximum: Option<u16>,
    #[codec(property_type = "PropertyType::ReasonString")]
    pub reason_string: Option<String>,
    #[codec(property_type = "PropertyType::SubIdAvail")]
    pub subscription_identifier_available: Option<bool>,
    #[codec(property_type = "PropertyType::UserProperty")]
    pub user_properties: UserProperty,
    #[codec(property_type = "PropertyType::WildcardSubAvail")]
    pub wildcard_subscription_available: Option<bool>,
    #[codec(property_type = "PropertyType::ShardSubAvail")]
    pub shared_subscription_available: Option<bool>,
    #[codec(property_type = "PropertyType::KeepAlive")]
    pub server_keep_alive: Option<u16>,
    #[codec(property_type = "PropertyType::RespInfo")]
    pub response_information: Option<String>,
    #[codec(property_type = "PropertyType::ServerReference")]
    pub server_reference: Option<String>,
    #[codec(property_type = "PropertyType::AuthMethod")]
    pub auth_method: Option<String>,
    #[codec(skip_if = "Vec::is_empty", property_type = "PropertyType::AuthData")]
    pub auth_data: Vec<u8>,
}

impl ConnAck {
    /// Returns true if the session is present flag is set in the CONNACK packet.
    ///
    pub fn session_present(&self) -> bool {
        (self.ack_flags & CONNACK_SESSION_PRESENT_MASK) != 0
    }

    /// Sets the session present flag in the CONNACK packet.
    pub fn set_session_present(&mut self, present: bool) {
        if present {
            self.ack_flags |= CONNACK_SESSION_PRESENT_MASK;
        } else {
            self.ack_flags &= !CONNACK_SESSION_PRESENT_MASK;
        }
    }

    pub fn set_session_expiry(&mut self, interval: u32) {
        if interval == 0 {
            self.session_expiry_interval = None;
            return;
        }
        self.session_expiry_interval = Some(interval);
    }

    pub fn set_server_keep_alive(&mut self, keep_alive: u16) {
        if keep_alive == 0 {
            self.server_keep_alive = None;
            return;
        }
        self.server_keep_alive = Some(keep_alive);
    }

    /// Gets the maximum number of QoS 1 and QoS 2 messages the session is willing to process.
    /// This is set as the CONNACK
    /// [Receive Maximum](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901049)
    /// property. The property is optional.
    pub fn receive_max(&self) -> Option<u16> {
        self.receive_maximum
    }

    /// Sets the maximum number of QoS 1 and QoS 2 messages the session is willing to process.
    /// This is set as the CONNACK
    /// [Receive Maximum](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901049)
    /// property.The property is optional and if set to 0, the property is cleared as it is a
    /// protocol error to have a value of 0.
    /// # Arguments
    /// max - The maximum number of QoS 1 and QoS 2 messages the session is willing to process.
    pub fn set_receive_max(&mut self, max: u16) {
        if max == 0 {
            self.receive_maximum = None;
            return;
        }
        self.receive_maximum = Some(max);
    }
}
