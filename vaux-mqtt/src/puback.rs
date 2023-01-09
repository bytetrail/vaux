use crate::{Decode, Encode, Size};

pub struct PubAck {}

impl Size for PubAck {
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

impl Encode for PubAck {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), crate::MQTTCodecError> {
        todo!()
    }
}

impl Decode for PubAck {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<(), crate::MQTTCodecError> {
        todo!()
    }
}
