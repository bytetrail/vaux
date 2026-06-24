use vaux_macro::packet;

use crate::{
    codec::{self, MqttCodecError},
    property::UserProperty,
    PropertyType, Reason,
};

#[packet(packet_type = "codec::PacketType::UnsubAck")]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UnsubAck {
    pub packet_id: u16,
    #[codec(property_type = "PropertyType::ReasonString")]
    pub reason: String,
    #[codec(property_type = "PropertyType::UserProperty")]
    pub user_properties: UserProperty,
    #[codec(payload_type = "remaining")]
    pub reason_code: Vec<Reason>,
}

#[packet(packet_type = "codec::PacketType::Unsubscribe")]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Unsubscribe {
    pub packet_id: u16,
    #[codec(property_type = "PropertyType::UserProperty")]
    pub props: UserProperty,
    #[codec(payload_type = "remaining")]
    pub topics: Vec<String>,
}

impl Unsubscribe {
    pub fn new(packet_id: u16, topics: Vec<String>) -> Self {
        Self {
            packet_id,
            topics: topics.to_vec(),
            ..Default::default()
        }
    }
    pub fn add_topic(&mut self, topic: String) {
        self.topics.push(topic);
    }
}
