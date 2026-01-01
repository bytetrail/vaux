pub mod codec;
pub mod connack;
pub mod connect;
pub mod disconnect;
pub mod fixed;
pub mod packet;
pub mod property;
pub mod publish;
pub mod pubresp;
pub mod subscribe;
pub mod test;
pub mod will;

use crate::codec::{put_bin, variable_byte_int_size};

use bytes::{Buf, BytesMut};
pub use codec::{MqttCodecError, Packet, PacketType, QoSLevel, Reason};
pub use {
    connack::ConnAck,
    connect::Connect,
    disconnect::Disconnect,
    fixed::FixedHeader,
    property::PropertyType,
    publish::Publish,
    pubresp::{PubAck, PubAckRecReason, PubComp, PubRec, PubRel, PubRelCompReason},
    subscribe::{Subscribe, SubscriptionFilter},
    will::{WillHeader, WillMessage},
};

pub trait PropertyCodecSize {
    fn property_size(&self) -> u32;
}

pub trait HeaderCodecSize: PropertyCodecSize {
    fn header_size(&self) -> u32;
}

pub trait PayloadCodecSize {
    fn payload_size(&self) -> u32;
}

pub trait PacketCodecSize: HeaderCodecSize + PayloadCodecSize {
    fn packet_size(&self) -> u32 {
        self.header_size() + self.payload_size()
    }
}

pub trait CodecSize {
    fn codec_size(&self) -> u32;
}

pub trait Encode {
    fn encode(&mut self, dest: &mut BytesMut) -> Result<(), MqttCodecError>;
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

impl Encode for Vec<u8> {
    fn encode(&mut self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        put_bin(self, dest)?;
        Ok(())
    }
}

impl Decode for Vec<u8> {
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MqttCodecError> {
        let idx = src.len() - src.remaining();
        self.extend(&src[idx..]);
        src.advance(src.len());
        Ok(())
    }
}

impl CodecSize for Vec<u8> {
    fn codec_size(&self) -> u32 {
        self.len() as u32
    }
}
