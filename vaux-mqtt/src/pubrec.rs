use std::collections::HashSet;

use bytes::{BufMut, Buf};

use crate::{Decode, Encode, Size, property::PropertyBundle, PropertyType, codec::{variable_byte_int_size, put_var_u32}, Reason, FixedHeader, PacketType};

const VARIABLE_HEADER_LEN: u32 = 2;

pub struct PubRec {
    pub reason: Reason,
    pub packet_id: u16,
    props: PropertyBundle,
}

impl PubRec {
    pub fn new() -> Self{
        let mut supported = HashSet::new();
        supported.insert(PropertyType::Reason);
        supported.insert(PropertyType::UserProperty);
        
        Self {
            reason: Reason::Success,
            packet_id: 0,
            props: PropertyBundle::new(supported),
        }
    }

    fn properties(&self) -> &PropertyBundle {
        &self.props
    }

    fn properties_mut(&mut self) -> &mut PropertyBundle {
        &mut self.props
    }
}

impl Size for PubRec {
    fn size(&self) -> u32 {
        let prop_size = self.property_size();
        let prop_size_len = variable_byte_int_size(prop_size);

        if prop_size == 0 {
            VARIABLE_HEADER_LEN
        } else {
            let remaining = 3 + prop_size + prop_size_len;
            VARIABLE_HEADER_LEN + remaining
        }
    }

    fn property_size(&self) -> u32 {
        self.props.size()
    }

    fn payload_size(&self) -> u32 {
        0
    }
}

impl Encode for PubRec {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), crate::MqttCodecError> {
        let mut header = FixedHeader::new(PacketType::PubRec);
        header.set_remaining(self.size());
        header.encode(dest)?;
        dest.put_u16(self.packet_id);
        if self.reason != Reason::Success {
            dest.put_u8(self.reason as u8);
        }
        put_var_u32(self.property_size(), dest);
        self.props.encode(dest)?;
        Ok(())
    }
}

impl Decode for PubRec {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<(), crate::MqttCodecError> {
        if src.remaining() == 2 {
            self.reason = Reason::Success;
            self.packet_id = src.get_u16();    
            return Ok(())
        } 
        self.packet_id = src.get_u16();    
        self.reason = Reason::try_from(src.get_u8())?;
        self.props.decode(src)?;

        Ok(())
    }
}
