mod codec;
mod connect;

use std::collections::HashMap;
pub use crate::codec::{MQTTCodec, MQTTCodecError, PacketType};
pub use crate::connect::Connect;

pub(crate) const PACKET_RESERVED_NONE: u8 = 0x00;
pub(crate) const PACKET_RESERVED_BIT1: u8 = 0x02;

#[derive(Debug, Eq, PartialEq)]
pub struct FixedHeader {
    packet_type: PacketType,
    flags: u8,
    remaining: u32,
}

impl FixedHeader {
    pub fn new(packet_type: PacketType) -> Self {
        match packet_type {
            PacketType::PubRel | PacketType::Subscribe | PacketType::Unsubscribe => FixedHeader {
                packet_type,
                flags: PACKET_RESERVED_BIT1,
                remaining: 0,
            },
            _ => FixedHeader {
                packet_type,
                flags: PACKET_RESERVED_NONE,
                remaining: 0,
            },
        }
    }

    pub fn packet_type(&self) -> PacketType {
        self.packet_type
    }

    pub fn set_remaining(&mut self, remaining: u32) {
        self.remaining = remaining;
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Packet {
    PingRequest(FixedHeader),
    PingResponse(FixedHeader),
    Connect(Connect),
    ConnAck(FixedHeader),
    Disconnect(FixedHeader),
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum QoSLevel {
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}

struct QoSParseError {}

impl TryFrom<u8> for QoSLevel {
    type Error = QoSParseError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(QoSLevel::AtMostOnce),
            0x01 => Ok(QoSLevel::AtLeastOnce),
            0x02 => Ok(QoSLevel::ExactlyOnce),
            _ => Err(QoSParseError {}),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct WillMessage {
    qos: QoSLevel,
    retain: bool,
    topic: String,
    payload: Vec<u8>,
    delay_interval: u32,
    expiry_interval: Option<u32>,
    payload_utf8: bool,
    content_type: Option<String>,
    response_topic: Option<String>,
    is_request: bool,
    correlation_data: Option<Vec<u8>>,
    user_property: Option<HashMap<String, String>>,
}

impl WillMessage {
    fn new(qos: QoSLevel, retain: bool) -> Self {
        WillMessage {
            qos,
            retain,
            topic: "".to_string(),
            payload: Vec::new(),
            delay_interval: 0,
            expiry_interval: None,
            payload_utf8: false,
            content_type: None,
            response_topic: None,
            is_request: false,
            correlation_data: None,
            user_property: None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
}
