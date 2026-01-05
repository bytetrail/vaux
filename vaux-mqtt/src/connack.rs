use crate::codec::{self, Reason};
use crate::packet::{ControlPacket, Empty};
use crate::property::{PropertyType, UserProperty};
use crate::{CodecSize, Decode, Encode, MqttCodecError, PropertyCodecSize, QoSLevel};
use vaux_macro::{CodecSize, Decode, Encode, PropertyCodecSize};

#[derive(Debug, Default, Clone, PartialEq, Eq, Decode, Encode, CodecSize, PropertyCodecSize)]
pub struct ConnAckHeader {
    ack_flags: u8,
    reason: Reason,
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

pub type ConnAck = ControlPacket<ConnAckHeader, Empty>;

impl ConnAck {
    pub fn reason(&self) -> Reason {
        self.variable_header.reason
    }

    pub fn set_reason(&mut self, reason: Reason) {
        self.variable_header.reason = reason;
    }

    pub fn session_expiry(&self) -> Option<u32> {
        self.variable_header.session_expiry_interval
    }

    pub fn set_session_expiry(&mut self, interval: u32) {
        if interval == 0 {
            self.variable_header.session_expiry_interval = None;
            return;
        }
        self.variable_header.session_expiry_interval = Some(interval);
    }

    pub fn assigned_client_id(&self) -> Option<String> {
        self.variable_header.assigned_client_id.clone()
    }

    pub fn set_assigned_client_id(&mut self, client_id: Option<String>) {
        self.variable_header.assigned_client_id = client_id;
    }

    pub fn maximum_qos(&self) -> Option<QoSLevel> {
        self.variable_header.maximum_qos
    }

    pub fn set_maximum_qos(&mut self, qos: QoSLevel) {
        self.variable_header.maximum_qos = Some(qos);
    }

    pub fn retain_available(&self) -> Option<bool> {
        self.variable_header.retain_available
    }

    pub fn set_retain_available(&mut self, available: bool) {
        self.variable_header.retain_available = Some(available);
    }

    pub fn server_keep_alive(&self) -> Option<u16> {
        self.variable_header.server_keep_alive
    }

    pub fn set_server_keep_alive(&mut self, keep_alive: u16) {
        if keep_alive == 0 {
            self.variable_header.server_keep_alive = None;
            return;
        }
        self.variable_header.server_keep_alive = Some(keep_alive);
    }

    /// Gets the maximum number of QoS 1 and QoS 2 messages the session is willing to process.
    /// This is set as the CONNACK
    /// [Receive Maximum](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901049)
    /// property. The property is optional.
    pub fn receive_max(&self) -> Option<u16> {
        self.variable_header.receive_maximum
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
            self.variable_header.receive_maximum = None;
            return;
        }
        self.variable_header.receive_maximum = Some(max);
    }
}
