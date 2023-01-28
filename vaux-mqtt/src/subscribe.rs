use bytes::{Buf, BufMut, BytesMut};

use crate::{
    codec::{
        decode_utf8_string, decode_variable_len_integer, encode_utf8_string, variable_byte_int_size,
    },
    Decode, Encode, FixedHeader, MQTTCodecError, PropertyType, QoSLevel, Size, UserPropertyMap,
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

impl TryFrom<u8> for RetainHandling {
    type Error = MQTTCodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(RetainHandling::Send),
            0x01 => Ok(RetainHandling::SendNew),
            0x02 => Ok(RetainHandling::None),
            v => Err(MQTTCodecError::new(
                format!("MQTTv5 3.8.3.1 invalid retain option: {}", v).as_str(),
            )),
        }
    }
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

    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError> {
        self.filter = decode_utf8_string(src)?;
        let flags = src.get_u8();
        self.qos = QoSLevel::try_from(flags & 0b_0000_0011)?;
        self.no_local = flags & 0b_0000_0100 == 0b_0000_0100;
        self.retain_as = flags & 0b_0000_1000 == 0b_0000_1000;
        self.handling = RetainHandling::try_from(flags & 0b_0011_0000 >> 4)?;

        Ok(())
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Subscribe {
    packet_id: u16,
    sub_id: Option<u32>,
    user_props: Option<UserPropertyMap>,
    payload: Vec<Subscription>,
}

impl Subscribe {

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

impl Decode for Subscribe {
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError> {
        if src.len() < 3 {
            return Err(MQTTCodecError::new(
                "MQTTv5 3.8.2 insufficient data for SUBSCRIBE",
            ));
        }
        self.packet_id = src.get_u16();
        let prop_len = decode_variable_len_integer(src);
        if src.remaining() < prop_len as usize {
            return Err(MQTTCodecError::new(
                "MQTTv5 3.8.2 insufficient data for SUBSCRIBE properties",
            ));
        }
        let read_until = src.remaining() - prop_len as usize;
        let mut sub_id_prop: bool = false;
        while src.remaining() > read_until {
            match PropertyType::try_from(src.get_u8()) {
                Ok(property_type) => {
                    if property_type != PropertyType::UserProperty {
                        match property_type {
                            PropertyType::SubscriptionId => {
                                if sub_id_prop {
                                    return Err(MQTTCodecError::new(
                                        "MQTTv5 3.8.2.1.2 subscription identifier repeated",
                                    ));
                                }
                                sub_id_prop = true;
                                self.sub_id = Some(decode_variable_len_integer(src));
                            }
                            _ => {
                                return Err(MQTTCodecError::new(
                                    "MQTTv5 3.8.2.1 unexpected property",
                                ))
                            }
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }
        while src.remaining() != 0 {
            let mut s = Subscription::default();
            s.decode(src)?;
            self.add_subscription(s);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use bytes::BytesMut;

    use crate::{Encode, QoSLevel, Size, Subscribe, PacketType, subscribe, Decode};

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
        let mut subscribe = Subscribe::default();
        subscribe.packet_id = 42;
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
        let mut subscribe = Subscribe::default();
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

    #[test] 
    fn test_basic_decode() {
        const ENCODED_PACKET: [u8; 43] = [
            0xba, 0xba,                 // packet id
            0x02,                       // property length
            0x0b, 0x0A,                 // subscription id
            0x00, 0x23, 0x2f, 0x66, 0x75, 0x73, 0x69, 0x6f, 0x6e, 0x2f, 0x75, 0x70, 0x64, 0x61, 0x74, 0x65, 0x2f, 0x66, 0x72, 
            0x69, 0x73, 0x63, 0x6f, 0x5f, 0x30, 0x31, 0x2f, 0x73, 0x65, 0x6e, 0x73, 0x6f, 0x72, 0x5f, 0x30, 0x31, 0x33,
            0b_0001_1101                // subscription flags
        ];
        const EXPECTED_PACKET_ID: u16 = 0xbaba;
        let mut src = BytesMut::from(&ENCODED_PACKET[..]);
        let mut subscribe = Subscribe::default();
        match subscribe.decode(&mut src) {
            Ok(_) => {
                assert_eq!(EXPECTED_PACKET_ID, subscribe.packet_id);
                assert_eq!(1, subscribe.payload.len());
            }
            Err(e) => panic!("unexpected error decoding publish: {}", e),
        }

    }
}
