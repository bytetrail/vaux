mod codec;
mod connack;
mod connect;
mod disconnect;
mod publish;
mod will;

use crate::codec::{
    encode_utf8_string, encode_variable_len_integer, variable_byte_int_size, PropertyType,
    PROP_SIZE_U32, PROP_SIZE_U8,
};

pub use crate::codec::{decode, decode_fixed_header, encode, MQTTCodecError, PacketType, Reason};
pub use crate::connack::ConnAck;
pub use crate::connect::Connect;
pub use crate::disconnect::Disconnect;
pub use crate::will::WillMessage;
use bytes::{BufMut, BytesMut};
use publish::Publish;
use std::collections::HashMap;

pub(crate) const PACKET_RESERVED_NONE: u8 = 0x00;
pub(crate) const PACKET_RESERVED_BIT1: u8 = 0x02;

pub trait Remaining {
    fn size(&self) -> u32;
    fn property_remaining(&self) -> Option<u32>;
    fn payload_remaining(&self) -> Option<u32>;
}

pub trait Encode: Remaining {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError>;
}

pub trait Decode {
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError>;
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UserPropertyMap {
    map: HashMap<String, Vec<String>>,
}

impl UserPropertyMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn map(&self) -> &HashMap<String, Vec<String>> {
        &self.map
    }

    pub fn add_property(&mut self, key: &str, value: &str) {
        if self.map.contains_key(key) {
            self.map.get_mut(key).unwrap().push(value.to_string());
        } else {
            let mut v: Vec<String> = Vec::new();
            v.push(value.to_string());
            self.map.insert(key.to_string(), v);
        }
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }
}

impl crate::Remaining for UserPropertyMap {
    fn size(&self) -> u32 {
        let mut remaining: u32 = 0;
        for (key, value) in self.map.iter() {
            let key_len = key.len() as u32 + 2;
            for v in value {
                remaining += key_len + v.len() as u32 + 3;
            }
        }
        remaining
    }

    fn property_remaining(&self) -> Option<u32> {
        None
    }

    fn payload_remaining(&self) -> Option<u32> {
        None
    }
}

impl Encode for UserPropertyMap {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        for (k, value) in self.map.iter() {
            for v in value {
                dest.put_u8(PropertyType::UserProperty as u8);
                encode_utf8_string(k, dest)?;
                encode_utf8_string(&v, dest)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
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

    pub fn flags(&self) -> u8 {
        self.flags
    }

    pub fn set_remaining(&mut self, remaining: u32) {
        self.remaining = remaining;
    }
}

impl crate::Remaining for FixedHeader {
    fn size(&self) -> u32 {
        self.remaining
    }

    fn property_remaining(&self) -> Option<u32> {
        None
    }

    fn payload_remaining(&self) -> Option<u32> {
        None
    }
}

impl Encode for FixedHeader {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        dest.put_u8(self.packet_type as u8 | self.flags);
        encode_variable_len_integer(self.remaining, dest);
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Packet {
    PingRequest(FixedHeader),
    PingResponse(FixedHeader),
    Connect(Connect),
    ConnAck(ConnAck),
    Publish(Publish),
    Disconnect(Disconnect),
}

impl From<&Packet> for PacketType {
    fn from(p: &Packet) -> Self {
        match p {
            Packet::PingRequest(_) => PacketType::PingReq,
            Packet::PingResponse(_) => PacketType::PingResp,
            Packet::Connect(_) => PacketType::Connect,
            Packet::ConnAck(_) => PacketType::ConnAck,
            Packet::Publish(_) => PacketType::Publish,
            Packet::Disconnect(_) => PacketType::Disconnect,
        }
    }
}

#[allow(clippy::enum_variant_names)]
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum QoSLevel {
    AtMostOnce = 0,
    AtLeastOnce = 1,
    ExactlyOnce = 2,
}

impl TryFrom<u8> for QoSLevel {
    type Error = MQTTCodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(QoSLevel::AtMostOnce),
            0x01 => Ok(QoSLevel::AtLeastOnce),
            0x02 => Ok(QoSLevel::ExactlyOnce),
            value => Err(MQTTCodecError::new(&format!(
                "{} is not a value QoSLevel",
                value
            ))),
        }
    }
}

#[cfg(test)]
mod test {}
