use bytes::{Buf, BufMut, BytesMut};

use crate::{
    codec::{
        decode_variable_byte_int, encode_variable_byte_int, ErrorKind, PACKET_RESERVED_BIT1,
        PACKET_RESERVED_NONE,
    },
    Decode, MqttCodecError, PacketType, QoSLevel,
};

const QOS_MASK: u8 = 0b_0000_0110;
const RETAIN_MASK: u8 = 0b_0000_0001;
const DUP_MASK: u8 = 0b_0000_1000;

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct FixedHeader {
    pub packet_type: PacketType,
    flags: u8,
    pub remaining: u32,
}

impl FixedHeader {
    pub fn new(packet_type: PacketType) -> Self {
        Self::new_with_remaining(packet_type, 0)
    }

    pub fn new_with_remaining(packet_type: PacketType, remaining: u32) -> Self {
        match packet_type {
            PacketType::PubRel | PacketType::Subscribe | PacketType::Unsubscribe => FixedHeader {
                packet_type,
                flags: PACKET_RESERVED_BIT1,
                remaining,
            },
            _ => FixedHeader {
                packet_type,
                flags: PACKET_RESERVED_NONE,
                remaining,
            },
        }
    }

    pub(crate) fn encode(&mut self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        dest.put_u8(self.packet_type as u8 | self.flags);
        encode_variable_byte_int(self.remaining, dest)?;
        Ok(())
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

    pub fn set_remaining(&mut self, remaining: u32) {
        self.remaining = remaining;
    }
}

// impl crate::Size for FixedHeader {
//     fn size(&self) -> u32 {
//         self.remaining
//     }
// }

// impl Encode for FixedHeader {
// fn encode(&mut self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
//     dest.put_u8(self.packet_type as u8 | self.flags);
//     put_var_u32(self.remaining, dest);
//     Ok(())
// }
// }

impl Decode for FixedHeader {
    fn decode(&mut self, src: &mut BytesMut) -> Result<u32, MqttCodecError> {
        if src.remaining() < 2 {
            return Err(MqttCodecError {
                reason: "insufficient data to decode fixed header".to_string(),
                kind: ErrorKind::InsufficientData(2, src.remaining()),
            });
        }
        for idx in 1..=3 {
            if src[idx] & 0x80 != 0x00 {
                // insufficient bytes left to read remaining
                if src.remaining() < 1 {
                    return Err(MqttCodecError {
                        reason: "insufficient data to decode fixed header".to_string(),
                        kind: ErrorKind::InsufficientData(idx + 1, src.remaining()),
                    });
                }
            } else {
                break;
            }
        }
        let first_byte = src.get_u8();
        let flags = first_byte & 0x0f;
        let (_bytes_read, packet_remaining) = decode_variable_byte_int(src)?;
        match src.remaining() {
            val if val < packet_remaining as usize => {
                return Err(MqttCodecError {
                    reason: format!(
                    "malformed packet: remaining length actual: {val} expected: {packet_remaining}",
                ),
                    kind: ErrorKind::InsufficientData(packet_remaining as usize, val),
                })
            }
            val if val > packet_remaining as usize => {
                let total = src.remaining();
                let index = total - (total - packet_remaining as usize);
                _ = src.split_off(index);
            }
            _ => {}
        }
        self.packet_type = match PacketType::try_from(first_byte & 0xF0) {
            Ok(pt) => pt,
            Err(_) => {
                return Err(MqttCodecError {
                    reason: "invalid packet type".to_string(),
                    kind: ErrorKind::MalformedPacket,
                })
            }
        };
        self.flags = first_byte & 0x0F;
        let (bytes_read, packet_remaining) = crate::codec::decode_variable_byte_int(src)?;
        match self.packet_type {
            PacketType::Connect
            | PacketType::PubRel
            | PacketType::PubAck
            | PacketType::Subscribe
            | PacketType::Unsubscribe
            | PacketType::ConnAck
            | PacketType::PubRec
            | PacketType::PubComp
            | PacketType::SubAck
            | PacketType::UnsubAck
            | PacketType::PingReq
            | PacketType::PingResp
            | PacketType::Disconnect
            | PacketType::Auth => {
                if flags != PACKET_RESERVED_NONE {
                    return Err(MqttCodecError::new(
                        format!("invalid flags for {}: {flags}", self.packet_type,).as_str(),
                    ));
                }
            }
            _ => {}
        }
        self.remaining = packet_remaining;
        self.flags = flags;
        Ok(1 + bytes_read)
    }
}
