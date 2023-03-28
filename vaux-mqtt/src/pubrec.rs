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
        if self.reason == Reason::Success && self.props.len() == 0 {
            Ok(())
        }else {
            dest.put_u8(self.reason as u8);
            self.props.encode(dest)?;
            Ok(())
        }
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



#[cfg(test)]
mod test {
    use bytes::BytesMut;

    use crate::property::Property;

    use super::*;
    
    #[test]
    fn basic_encode() {
        const EXPECTED_LEN: usize = 4;
        let mut pubrec = PubRec::new();
        pubrec.packet_id = 12345;
        pubrec.reason = Reason::Success;
        let mut dest = BytesMut::new();
        let result = pubrec.encode(&mut dest);
        assert!(result.is_ok());
        assert_eq!(EXPECTED_LEN,  dest.len());
    }

    #[test]
    fn encode_with_reason() {
        const EXPECTED_LEN: usize = 25;
        let mut pubrec = PubRec::new();
        pubrec.packet_id = 12345;
        pubrec.reason = Reason::UnspecifiedErr;
        pubrec.properties_mut().set_property(Property::Reason("unable to comply".to_string()));

        let mut dest = BytesMut::new();
        let result = pubrec.encode(&mut dest);
        assert!(result.is_ok());
        assert_eq!(EXPECTED_LEN,  dest.len());
    }


}