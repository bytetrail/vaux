use bytes::{BufMut, BytesMut};

use crate::{codec::{encode_variable_len_integer, PACKET_RESERVED_BIT1, PACKET_RESERVED_NONE}, Encode, MQTTCodecError, PacketType};



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
