use crate::{Decode, Encode, Size};

pub struct PubRec {}

impl Size for PubRec {
    fn size(&self) -> u32 {
        todo!()
    }

    fn property_size(&self) -> u32 {
        todo!()
    }

    fn payload_size(&self) -> u32 {
        todo!()
    }
}

impl Encode for PubRec {
    fn encode(&self, _dest: &mut bytes::BytesMut) -> Result<(), crate::MQTTCodecError> {
        todo!()
    }
}

impl Decode for PubRec {
    fn decode(&mut self, _src: &mut bytes::BytesMut) -> Result<(), crate::MQTTCodecError> {
        todo!()
    }
}
