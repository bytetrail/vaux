use std::fmt::{Display, Formatter};
use std::io::Error;
use bytes::BytesMut;
use tokio_util::codec::Decoder;

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
            _  => PacketType::Auth,
        }
    }
}



struct ControlPacket {
    packet_type: PacketType,
    flags: u8,
    remaining: u32,
}

impl ControlPacket {
}

#[derive(Debug)]
struct MQTTCodecError {
    reason: String,
}

impl Display for MQTTCodecError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl From<std::io::Error> for MQTTCodecError {
    fn from(err: std::io::Error) -> Self {
        MQTTCodecError {
            reason: "".to_string()
        }
    }
}

impl MQTTCodecError {
    pub fn new(reason: &str) -> Self {
        MQTTCodecError { reason: reason.to_string() }
    }
}

struct MQTTCodec {}

impl Decoder for MQTTCodec {
    type Item = ControlPacket;
    type Error = MQTTCodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let packet_type = PacketType::from(src[0]);
        match packet_type {
            PacketType::PingReq => Ok(Some(ControlPacket{
                packet_type,
                flags: src[0] & 0x0f,
                remaining: 0
            })),
            _ => Err(MQTTCodecError::new("unexpeccted packet type"))
        }
    }
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_control_packet_type_from() {
        let val = 0x12;
        assert_eq!(PacketType::Connect, PacketType::from(val), "expected {:?}", PacketType::Connect);
        let val = 0xaa;
        assert_eq!(PacketType::Unsubscribe, PacketType::from(val), "expected {:?}", PacketType::Unsubscribe);
    }
}
