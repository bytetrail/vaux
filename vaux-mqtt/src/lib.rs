pub mod codec;
pub mod connack;
pub mod connect;
pub mod disconnect;
pub mod property;
pub mod publish;
pub mod pubresp;
pub mod subscribe;
pub mod unsubscribe;
pub mod test;
pub mod will;

pub use codec::{MqttCodecError, Packet, PacketType, QoSLevel, Reason};
use vaux_macro::packet;
use crate::codec::{Encode, PropertyEncode, PropertyCodecSize, Decode};

pub use {
    connack::ConnAck,
    connect::Connect,
    disconnect::Disconnect,
    property::PropertyType,
    publish::{PayloadFormat, Publish},
    pubresp::{PubAck, PubComp, PubRec, PubRel},
    subscribe::{Subscribe, SubscriptionFilter},
    will::{WillHeader, WillMessage},
    codec::fixed::{FixedHeader},
};

pub enum MqttVersion {
    V3,
    V5,
}

impl std::fmt::Display for MqttVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            MqttVersion::V3 => write!(f, "v3.1.1"),
            MqttVersion::V5 => write!(f, "v5.0"),
        }
    }
}

pub struct MqttError {
    pub version: Option<MqttVersion>,
    pub section: Option<String>,
    pub message: String,
}

impl MqttError {
    pub fn new(message: &str) -> MqttError {
        MqttError {
            version: None,
            section: None,
            message: message.to_string(),
        }
    }

    pub fn new_from_spec(version: MqttVersion, section: &str, message: &str) -> MqttError {
        MqttError {
            version: Some(version),
            section: Some(section.to_string()),
            message: message.to_string(),
        }
    }
}

impl std::fmt::Display for MqttError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(version) = &self.version {
            write!(f, "MQTT{version} ")?;
        }
        if let Some(section) = &self.section {
            write!(f, " {section}: ")?;
        }
        write!(f, "{}", self.message)
    }
}


#[packet(packet_type = "codec::PacketType::PingReq")]
#[derive( Debug, Clone, Eq, PartialEq)]
pub struct PingReq;

impl Default for PingReq {
    fn default() -> Self {
        Self {
            fixed_header: codec::FixedHeader::new(codec::PacketType::PingReq),
        }
    }
}


#[packet(packet_type = "codec::PacketType::PingResp")]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PingResp;

impl Default for PingResp {
    fn default() -> Self {
        Self {
            fixed_header: codec::FixedHeader::new(codec::PacketType::PingResp),
        }
    }
}