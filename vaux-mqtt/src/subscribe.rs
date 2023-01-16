use bytes::{BufMut, BytesMut};

use crate::{
    codec::{encode_utf8_string, variable_byte_int_size},
    Encode, FixedHeader, MQTTCodecError, QoSLevel, Size, UserPropertyMap,
};

const VAR_HDR_LEN: u32 = 2;

/// MQTT v5 3.8.3.1 Subscription Options
/// bits 4 and 5 of the subscription options hold the retain handling flag.
/// Retain handling is used to determine how messages published with the
/// retain flag set to ```true``` are handled when the ```SUBSCRIBE``` packet
/// is received.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum RetainHandling {
    #[default]
    Send,
    SendNew,
    None,
}

/// Subscription represents an MQTT v5 3.8.3 SUBSCRIBE Payload. The MQTT v5
/// 3.8.3.1 options are represented as individual fields in the struct.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Subscription {
    filter: String,
    qos: QoSLevel,
    no_local: bool,
    retain_as: bool,
    handling: RetainHandling,
}

impl Subscription {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        encode_utf8_string(&self.filter, dest)?;
        let flags = self.qos as u8
            | (self.no_local as u8) << 2
            | (self.retain_as as u8) << 3
            | (self.handling as u8) << 4;
        dest.put_u8(flags);

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscribe {
    packet_id: u16,
    sub_id: Option<u32>,
    user_props: Option<UserPropertyMap>,
    payload: Vec<Subscription>,
}

impl Subscribe {
    pub fn new(packet_id: u16) -> Result<Self, MQTTCodecError> {
        if packet_id == 0 {
            return Err(MQTTCodecError::new(
                "MQTTv5 2.2.1 packet identifier must not be 0",
            ));
        }
        Ok(Subscribe {
            packet_id,
            sub_id: None,
            user_props: None,
            payload: Vec::new(),
        })
    }

    fn add_subscription(&mut self, subscription: Subscription) {
        self.payload.push(subscription);
    }

    fn encode_payload(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        if self.payload.is_empty() {
            return Err(MQTTCodecError::new(
                "MQTTv5 3.8.3 subscribe payload must exist",
            ));
        }
        for sub in &self.payload {
            sub.encode(dest)?;
        }
        Ok(())
    }
}

impl Size for Subscribe {
    fn size(&self) -> u32 {
        let prop_size = self.property_size();
        VAR_HDR_LEN + prop_size + variable_byte_int_size(prop_size) + self.payload_size()
    }

    fn property_size(&self) -> u32 {
        self.sub_id.map_or(0, |id| variable_byte_int_size(id))
            + self.user_props.as_ref().map_or(0, |p| p.size())
    }

    fn payload_size(&self) -> u32 {
        let mut remaining = 0;
        for s in &self.payload {
            remaining += s.filter.len() + 3;
        }
        remaining as u32
    }
}

impl Encode for Subscribe {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        if self.packet_id == 0 {
            return Err(MQTTCodecError::new(
                "MQTTv5 2.2.1 packet identifier must not be 0",
            ));
        }
        let mut hdr = FixedHeader::new(crate::PacketType::Subscribe);
        hdr.set_remaining(self.size());
        hdr.encode(dest)?;
        dest.put_u16(self.packet_id);
        if let Some(props) = self.user_props.as_ref() {
            props.encode(dest)?;    
        }
        self.encode_payload(dest)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use bytes::BytesMut;

    use crate::{QoSLevel, Size, Subscribe, Encode};

    use super::{RetainHandling, Subscription};

    #[test]
    fn test_encode_flags() {
        const EXPECTED_FLAG: u8 = 0b_0010_1110;
        const EXPECTED_LEN: usize = 7;
        let mut sub = Subscription::default();
        sub.filter = "test".to_string();
        sub.qos = QoSLevel::ExactlyOnce;
        sub.no_local = true;
        sub.retain_as = true;
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

    #[test]
    fn test_payload_size() {
        const EXPECTED_PAYLOAD_SIZE: u32 = 7;
        let mut subscribe = Subscribe::new(42).unwrap();
        let subscription = Subscription {
            filter: "test".to_string(),
            qos: QoSLevel::AtLeastOnce,
            retain_as: false,
            no_local: false,
            handling: RetainHandling::None,
        };
        subscribe.add_subscription(subscription);
        let payload_remaining = subscribe.payload_size();
        assert!(payload_remaining > 0);
        assert_eq!(EXPECTED_PAYLOAD_SIZE, payload_remaining);
    }

    #[test]
    fn test_bad_packet_id() {
        assert!(Subscribe::new(0).is_err());
        let mut subscribe = Subscribe::new(42).unwrap();
        subscribe.packet_id = 0;
        let subscription = Subscription {
            filter: "test".to_string(),
            qos: QoSLevel::AtLeastOnce,
            retain_as: false,
            no_local: false,
            handling: RetainHandling::None,
        };
        subscribe.add_subscription(subscription);
        let mut dest = BytesMut::new();
        match subscribe.encode(&mut dest) {
            Ok(_) => panic!("expected MQTT encoding error"),
            Err(e) => {
                assert!(e.reason.starts_with("MQTTv5 2.2.1"));
            }
        }
    }

}
