use bytes::{BufMut, BytesMut};

use crate::{
    codec::encode_utf8_string, Encode, MQTTCodecError, QoSLevel, Remaining, UserPropertyMap,
};

#[repr(u8)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
enum RetainHandling {
    #[default]
    Send,
    SendNew,
    None,
}

#[derive(Debug, Default)]
pub struct Subscription {
    filter: String,
    qos: QoSLevel,
    no_local: bool,
    retain_as_pub: bool,
    handling: RetainHandling,
}

impl Subscription {
    fn encode(&self, dest: &mut BytesMut) {
        encode_utf8_string(&self.filter, dest);
        let flags = self.qos as u8
            | (self.no_local as u8) << 2
            | (self.retain_as_pub as u8) << 3
            | (self.handling as u8) << 4;
        dest.put_u8(flags);
    }
}

pub struct Subscribe {
    packet_id: u16,
    sub_id: Option<u32>,
    user_props: Option<UserPropertyMap>,
    payload: Vec<Subscription>,
}

impl Subscribe {
    pub fn new(packet_id: u16) -> Self {
        Subscribe {
            packet_id,
            sub_id: None,
            user_props: None,
            payload: Vec::new(),
        }
    }

    fn encode_payload(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        if self.payload.is_empty() {
            return Err(MQTTCodecError::new(
                "MQTTv5 3.8.3 subscribe payload must exist",
            ));
        }
        for sub in &self.payload {
            sub.encode(dest);
        }
        Ok(())
    }
}

impl Remaining for Subscribe {
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

impl Encode for Subscribe {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        self.encode_payload(dest);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use bytes::BytesMut;

    use crate::QoSLevel;

    use super::{RetainHandling, Subscription};

    #[test]
    fn test_encode_flags() {
        const EXPECTED_FLAG: u8 = 0b_0010_1110;
        const EXPECTED_LEN: usize = 7;
        let mut sub = Subscription::default();
        sub.filter = "test".to_string();
        sub.qos = QoSLevel::ExactlyOnce;
        sub.no_local = true;
        sub.retain_as_pub = true;
        sub.handling = RetainHandling::None;

        let mut dest = BytesMut::new();
        sub.encode(&mut dest);
        assert_eq!(EXPECTED_LEN, dest.len());
        assert_eq!(EXPECTED_FLAG, dest[6]);

        const SEND_EXPECTED_FLAG: u8 = 0b_0001_1110;
        sub.handling = RetainHandling::SendNew;
        let mut dest = BytesMut::new();
        sub.encode(&mut dest);
        assert_eq!(EXPECTED_LEN, dest.len());
        assert_eq!(SEND_EXPECTED_FLAG, dest[6]);

        const QOS_EXPECTED_FLAG: u8 = 0b_0000_1101;
        sub.qos = QoSLevel::AtLeastOnce;
        sub.handling = RetainHandling::Send;
        let mut dest = BytesMut::new();
        sub.encode(&mut dest);
        assert_eq!(EXPECTED_LEN, dest.len());
        assert_eq!(QOS_EXPECTED_FLAG, dest[6]);
    }
}
