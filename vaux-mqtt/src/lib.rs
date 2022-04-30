mod codec;
mod connect;
mod connack;

use std::collections::HashMap;
use bytes::{BufMut, BytesMut};
pub use crate::codec::{MQTTCodec, MQTTCodecError, PacketType};
use crate::codec::{encode_variable_len_integer, PROP_SIZE_U32, PROP_SIZE_U8, variable_byte_int_size};
pub use crate::connect::Connect;
pub use crate::connack::{Reason, ConnAck};

pub(crate) const PACKET_RESERVED_NONE: u8 = 0x00;
pub(crate) const PACKET_RESERVED_BIT1: u8 = 0x02;


const DEFAULT_WILL_DELAY: u32 = 0;

pub(crate) trait Sized {
    fn size(&self) -> u32;
}

pub(crate) trait Encode: Sized {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError>;
}

pub(crate) trait Decode {
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError>;
}

type UserProperty = HashMap<String, String>;

impl crate::Sized for UserProperty {
    fn size(&self) -> u32 {
        let mut remaining: u32  = 0;
        for (key, value) in self.iter() {
            remaining += key.len() as u32 + 2 + value.len() as u32 + 3;
        }
        remaining
    }
}

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

impl crate::Sized for FixedHeader {
    fn size(&self) -> u32 { self.remaining }
}

impl Encode for FixedHeader {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        dest.put_u8(self.packet_type as u8 | self.flags);
        encode_variable_len_integer(self.remaining, dest);
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Packet {
    PingRequest(FixedHeader),
    PingResponse(FixedHeader),
    Connect(Connect),
    ConnAck(ConnAck),
    Disconnect(FixedHeader),
}

impl crate::Sized for Packet {
    fn size(&self) -> u32 {
        match self {
            Packet::ConnAck(c) => c.size(),
            _ => 0
        }
    }
}

impl Encode for Packet {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        match self {
            Packet::ConnAck(c) => c.encode(dest),
            Packet::PingRequest(h) |
            Packet::PingResponse(h) => h.encode(dest),
            _ => Err(MQTTCodecError::new("unsupported packet type"))
        }
    }
}



#[allow(clippy::enum_variant_names)]
#[repr(u8)]
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

impl crate::Sized for WillMessage {
    fn size(&self) -> u32 {
        let mut remaining = 0;
        if self.delay_interval != DEFAULT_WILL_DELAY {
            remaining += PROP_SIZE_U32;
        }
        if self.payload_utf8 {
            remaining += PROP_SIZE_U8;
        }
        if self.expiry_interval != None {
            remaining += PROP_SIZE_U32;
        }
        if let Some(content_type) = &self.content_type {
            remaining += content_type.len() as u32 + 3;
        }
        if let Some(response_topic) = &self.response_topic {
            remaining += response_topic.len() as u32 + 3;
        }
        if let Some(correlation_data) = &self.correlation_data {
            remaining += correlation_data.len() as u32 + 3;
        }
        if let Some(user_property) = &self.user_property {
            remaining += user_property.size();
        }
        remaining += variable_byte_int_size(remaining);
        remaining
    }
}

#[cfg(test)]
mod test {
}
