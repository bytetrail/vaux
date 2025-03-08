use bytes::{BufMut, BytesMut};

use crate::{
    codec::{put_var_u32, ErrorKind, PACKET_RESERVED_BIT1, PACKET_RESERVED_NONE},
    Encode, MqttCodecError, PacketType, QoSLevel,
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

    pub fn packet_type(&self) -> PacketType {
        self.packet_type
    }

    pub fn dup(&self) -> bool {
        (self.flags & DUP_MASK) != 0
    }

    pub fn set_dup(&mut self, dup: bool) {
        self.flags = self.flags & !DUP_MASK | (dup as u8) << 3;
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
        self.flags = self.flags & !QOS_MASK | (qos as u8) << 1;
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

impl crate::Size for FixedHeader {
    fn size(&self) -> u32 {
        self.remaining
    }

    fn property_size(&self) -> u32 {
        0
    }

    fn payload_size(&self) -> u32 {
        0
    }
}

impl Encode for FixedHeader {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        dest.put_u8(self.packet_type as u8 | self.flags);
        put_var_u32(self.remaining, dest);
        Ok(())
    }
}
