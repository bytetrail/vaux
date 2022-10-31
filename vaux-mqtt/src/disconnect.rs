use crate::{Decode, Encode, Reason, Remaining};

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Disconnect {
    reason: Reason,
}

impl Disconnect {
    pub fn new(reason: Reason) -> Self {
        Self { reason }
    }
}

impl Remaining for Disconnect {
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

impl Encode for Disconnect {
    fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), crate::MQTTCodecError> {
        todo!()
    }
}

impl Decode for Disconnect {
    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<(), crate::MQTTCodecError> {
        todo!()
    }
}
