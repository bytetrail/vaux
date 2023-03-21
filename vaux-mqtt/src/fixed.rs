use bytes::{BufMut, BytesMut};

use crate::{
    codec::{put_var_u32, PACKET_RESERVED_BIT1, PACKET_RESERVED_NONE},
    Encode, MqttCodecError, PacketType,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FixedHeader {
    pub packet_type: PacketType,
    pub flags: u8,
    pub remaining: u32,
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
