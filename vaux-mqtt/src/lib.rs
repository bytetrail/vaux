pub mod codec;

pub use crate::codec::{MQTTCodec, MQTTCodecError};
use std::fmt::{Display, Formatter};

const PACKET_RESERVED_NONE: u8 = 0x00;
const PACKET_RESERVED_BIT1: u8 = 0x02;

pub trait VariableHeader {
    fn transport_size() -> u32;
}

pub trait Payload {
    fn transport_size() -> u32;
}

pub trait Packet {
    fn remaining() -> u32;
}

/// MQTT Control Packet Type
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum PacketType {
    Connect = 0x10,
    ConnAck = 0x20,
    Publish = 0x30,
    PubAck = 0x40,
    PubRec = 0x50,
    PubRel = 0x60,
    PubComp = 0x70,
    Subscribe = 0x80,
    SubAck = 0x90,
    Unsubscribe = 0xa0,
    UnsubAck = 0xb0,
    PingReq = 0xc0,
    PingResp = 0xd0,
    Disconnect = 0xe0,
    Auth = 0xf0,
}

impl From<u8> for PacketType {
    fn from(val: u8) -> Self {
        match val & 0xf0 {
            0x10 => PacketType::Connect,
            0x20 => PacketType::ConnAck,
            0x30 => PacketType::Publish,
            0x40 => PacketType::PubAck,
            0x50 => PacketType::PubRec,
            0x60 => PacketType::PubRel,
            0x70 => PacketType::PubComp,
            0x80 => PacketType::Subscribe,
            0x90 => PacketType::SubAck,
            0xa0 => PacketType::Unsubscribe,
            0xb0 => PacketType::UnsubAck,
            0xc0 => PacketType::PingReq,
            0xd0 => PacketType::PingResp,
            0xe0 => PacketType::Disconnect,
            _ => PacketType::Auth,
        }
    }
}

impl Display for PacketType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{:?}", &self).as_str().to_uppercase())
    }
}

#[derive(Debug)]
pub struct ControlPacket {
    packet_type: PacketType,
    flags: u8,
    remaining: u32,
}

impl ControlPacket {
    pub fn new(packet_type: PacketType) -> Self {
        match packet_type {
            PacketType::PubRel | PacketType::Subscribe | PacketType::Unsubscribe => ControlPacket {
                packet_type,
                flags: PACKET_RESERVED_BIT1,
                remaining: 0,
            },
            _ => ControlPacket {
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

#[cfg(test)]
mod Test {
    use super::*;

    #[test]
    /// Random test of display trait for packet types.
    fn test_packet_type_display() {
        let p = PacketType::UnsubAck;
        assert_eq!("UNSUBACK", format!("{}", p));
        let p = PacketType::PingReq;
        assert_eq!("PINGREQ", format!("{}", p));
    }

    #[test]
    fn test_control_packet_type_from() {
        let val = 0x12;
        assert_eq!(
            PacketType::Connect,
            PacketType::from(val),
            "expected {:?}",
            PacketType::Connect
        );
        let val = 0x2f;
        assert_eq!(
            PacketType::ConnAck,
            PacketType::from(val),
            "expected {:?}",
            PacketType::ConnAck
        );
        let val = 0x35;
        assert_eq!(
            PacketType::Publish,
            PacketType::from(val),
            "expected {:?}",
            PacketType::Publish
        );
        let val = 0x47;
        assert_eq!(
            PacketType::PubAck,
            PacketType::from(val),
            "expected {:?}",
            PacketType::PubAck
        );
        let val = 0x5f;
        assert_eq!(
            PacketType::PubRec,
            PacketType::from(val),
            "expected {:?}",
            PacketType::PubRec
        );
        let val = 0x6f;
        assert_eq!(
            PacketType::PubRel,
            PacketType::from(val),
            "expected {:?}",
            PacketType::PubRel
        );
        let val = 0x7f;
        assert_eq!(
            PacketType::PubComp,
            PacketType::from(val),
            "expected {:?}",
            PacketType::PubComp
        );
        let val = 0x8f;
        assert_eq!(
            PacketType::Subscribe,
            PacketType::from(val),
            "expected {:?}",
            PacketType::Subscribe
        );
        let val = 0x9f;
        assert_eq!(
            PacketType::SubAck,
            PacketType::from(val),
            "expected {:?}",
            PacketType::SubAck
        );
        let val = 0xaf;
        assert_eq!(
            PacketType::Unsubscribe,
            PacketType::from(val),
            "expected {:?}",
            PacketType::Unsubscribe
        );
        let val = 0xbf;
        assert_eq!(
            PacketType::UnsubAck,
            PacketType::from(val),
            "expected {:?}",
            PacketType::UnsubAck
        );
        let val = 0xcf;
        assert_eq!(
            PacketType::PingReq,
            PacketType::from(val),
            "expected {:?}",
            PacketType::PingReq
        );
        let val = 0xdf;
        assert_eq!(
            PacketType::PingResp,
            PacketType::from(val),
            "expected {:?}",
            PacketType::PingResp
        );
        let val = 0xff;
        assert_eq!(
            PacketType::Auth,
            PacketType::from(val),
            "expected {:?}",
            PacketType::Auth
        );
    }
}
