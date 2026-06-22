use bytes::{Buf, BufMut, BytesMut};

use crate::{
    codec::{Decode, Encode, ErrorKind, PACKET_RESERVED_BIT1, PACKET_RESERVED_NONE},
    MqttCodecError, PacketType, QoSLevel,
};

const QOS_MASK: u8 = 0b_0000_0110;
const RETAIN_MASK: u8 = 0b_0000_0001;
const DUP_MASK: u8 = 0b_0000_1000;

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct FixedHeader {
    pub packet_type: PacketType,
    flags: u8,
}

impl FixedHeader {
    pub fn new(packet_type: PacketType) -> Self {
        match packet_type {
            PacketType::PubRel | PacketType::Subscribe | PacketType::Unsubscribe => FixedHeader {
                packet_type,
                flags: PACKET_RESERVED_BIT1,
            },
            _ => FixedHeader {
                packet_type,
                flags: PACKET_RESERVED_NONE,
            },
        }
    }

    pub fn packet_type(&self) -> PacketType {
        self.packet_type
    }

    pub fn dup(&self) -> bool {
        (self.flags & DUP_MASK) != 0
    }

    pub fn set_dup(&mut self, dup: bool) {
        self.flags = self.flags & !DUP_MASK | ((dup as u8) << 3)
    }

    pub fn retain(&self) -> bool {
        (self.flags & RETAIN_MASK) != 0
    }

    pub fn set_retain(&mut self, retain: bool) {
        self.flags = self.flags & !RETAIN_MASK | retain as u8;
    }

    pub fn qos(&self) -> QoSLevel {
        ((self.flags & QOS_MASK) >> 1).try_into().unwrap()
    }

    pub fn set_qos(&mut self, qos: QoSLevel) {
        self.flags = self.flags & !QOS_MASK | ((qos as u8) << 1);
    }

    pub fn flags(&self) -> u8 {
        self.flags
    }

    pub fn set_flags(&mut self, flags: u8) -> Result<(), MqttCodecError> {
        if flags & QOS_MASK == QOS_MASK {
            return Err(MqttCodecError {
                reason: "unsupported QOS level".to_string(),
                kind: ErrorKind::UnsupportedQosLevel,
            });
        }
        self.flags = flags;
        Ok(())
    }

    pub fn clear_flags(&mut self) {
        self.flags = 0;
    }
}

impl Encode for FixedHeader {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        let first_byte = (self.packet_type as u8) | (self.flags & 0x0f);
        dest.put_u8(first_byte);
        Ok(())
    }
}

impl Decode for FixedHeader {
    fn decode(&mut self, src: &mut BytesMut) -> Result<usize, MqttCodecError> {
        if src.remaining() < 2 {
            return Err(MqttCodecError::new_with_kind(
                "Insufficient data",
                ErrorKind::InsufficientData(2, src.remaining()),
            ));
        }

        let first_byte = src.get_u8();
        let flags = first_byte & 0x0f;
        self.packet_type = match PacketType::try_from(first_byte & 0xF0) {
            Ok(pt) => pt,
            Err(_) => {
                return Err(MqttCodecError {
                    reason: "invalid packet type".to_string(),
                    kind: ErrorKind::MalformedPacket,
                })
            }
        };
        match self.packet_type {
            PacketType::PubRel | PacketType::Subscribe | PacketType::Unsubscribe
                if flags != PACKET_RESERVED_BIT1 =>
            {
                return Err(MqttCodecError::new(
                    format!("invalid flags for {}: {flags}", self.packet_type).as_str(),
                ));
            }
            PacketType::Connect
            | PacketType::PubAck
            | PacketType::ConnAck
            | PacketType::PubRec
            | PacketType::PubComp
            | PacketType::SubAck
            | PacketType::UnsubAck
            | PacketType::PingReq
            | PacketType::PingResp
            | PacketType::Disconnect
            | PacketType::Auth
                if flags != PACKET_RESERVED_NONE =>
            {
                return Err(MqttCodecError::new(
                    format!("invalid flags for {}: {flags}", self.packet_type).as_str(),
                ));
            }
            _ => {}
        }
        self.flags = flags;
        Ok(1)
    }
}
