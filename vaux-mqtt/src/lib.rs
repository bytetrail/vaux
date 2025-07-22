pub mod codec;
pub mod connack;
pub mod connect;
pub mod disconnect;
pub mod fixed;
pub mod property;
pub mod publish;
pub mod pubresp;
pub mod subscribe;
pub mod test;
mod will;

use crate::codec::{put_utf8, variable_byte_int_size};

pub use crate::property::PropertyType;

pub use crate::codec::{
    decode, decode_fixed_header, encode, MqttCodecError, Packet, PacketType, QoSLevel, Reason,
};
pub use crate::connack::ConnAck;
pub use crate::connect::Connect;
pub use crate::will::WillMessage;
pub use crate::{
    disconnect::Disconnect, fixed::FixedHeader, pubresp::PubResp, subscribe::Subscribe,
    subscribe::SubscriptionFilter,
};
use bytes::BytesMut;
#[macro_use]
extern crate lazy_static;

pub trait Size {
    fn size(&self) -> u32;
    fn property_size(&self) -> u32;
    fn payload_size(&self) -> u32;
}

pub trait Encode: Size {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError>;
}

pub trait Decode {
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MqttCodecError>;
}

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

//pub type Result<T> = std::result::Result<T, MqttError>;
