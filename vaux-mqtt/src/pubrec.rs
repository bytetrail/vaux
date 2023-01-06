use crate::{Decode, Encode, Remaining};

pub struct PubRec {}

impl Remaining for PubRec {
    fn size(&self) -> u32 {
        todo!()
    }

    fn property_remaining(&self) -> Option<u32> {
        todo!()
    }

    fn payload_remaining(&self) -> Option<u32> {
        todo!()
    }
}

impl Encode for PubRec {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), crate::MQTTCodecError> {
        todo!()
    }
}

impl Decode for PubRec {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<(), crate::MQTTCodecError> {
        todo!()
    }
}
