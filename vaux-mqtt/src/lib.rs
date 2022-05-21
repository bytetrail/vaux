mod codec;
mod connack;
mod connect;
mod will;

use crate::codec::{
    encode_utf8_string, encode_variable_len_integer, variable_byte_int_size, PropertyType,
    PROP_SIZE_U32, PROP_SIZE_U8,
};

pub use crate::will::WillMessage;
pub use crate::codec::{MQTTCodec, MQTTCodecError, PacketType, Reason};
pub use crate::connack::ConnAck;
pub use crate::connect::Connect;
use bytes::{BufMut, BytesMut};
use std::collections::HashMap;

pub(crate) const PACKET_RESERVED_NONE: u8 = 0x00;
pub(crate) const PACKET_RESERVED_BIT1: u8 = 0x02;

pub(crate) trait Remaining {
    fn size(&self) -> u32;
    fn property_remaining(&self) -> Option<u32>;
    fn payload_remaining(&self) -> Option<u32>;
}

pub(crate) trait Encode: Remaining {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError>;
}

pub(crate) trait Decode {
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError>;
}

type UserPropertyMap = HashMap<String, String>;

impl crate::Remaining for UserPropertyMap {
    fn size(&self) -> u32 {
        let mut remaining: u32 = 0;
        for (key, value) in self.iter() {
            remaining += key.len() as u32 + 2 + value.len() as u32 + 3;
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
        for (k, v) in self.iter() {
            dest.put_u8(PropertyType::UserProperty as u8);
            encode_utf8_string(k, dest)?;
            encode_utf8_string(v, dest)?;
        }
        Ok(())
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

#[derive(Debug, Eq, PartialEq)]
pub enum Packet {
    PingRequest(FixedHeader),
    PingResponse(FixedHeader),
    Connect(Connect),
    ConnAck(ConnAck),
    Disconnect(FixedHeader),
}

#[allow(clippy::enum_variant_names)]
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum QoSLevel {
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}

impl TryFrom<u8> for QoSLevel {
    type Error = MQTTCodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(QoSLevel::AtMostOnce),
            0x01 => Ok(QoSLevel::AtLeastOnce),
            0x02 => Ok(QoSLevel::ExactlyOnce),
            value => Err(MQTTCodecError::new(&format!("{} is not a value QoSLevel", value))),
        }
    }
}


#[cfg(test)]
mod test {}
