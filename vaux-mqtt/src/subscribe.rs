use bytes::{Buf, BufMut, BytesMut};

use crate::{
    codec::{
        decode_utf8_string, decode_variable_len_integer, encode_utf8_string,
        encode_var_int_property, encode_variable_len_integer, variable_byte_int_size,
        PROP_SIZE_UTF8_STRING,
    },
    Decode, Encode, FixedHeader, MQTTCodecError, PropertyType, QoSLevel, Reason, Size,
    UserPropertyMap,
};

const VAR_HDR_LEN: u32 = 2;
const PROP_ID_LEN: u32 = 1;

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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SubAck {
    packet_id: u16,
    reason_desc: Option<String>,
    user_props: Option<UserPropertyMap>,
    sub_reason: Vec<Reason>,
}

impl Size for SubAck {
    fn size(&self) -> u32 {
        let prop_size = self.property_size();
        VAR_HDR_LEN + variable_byte_int_size(prop_size) + prop_size + self.payload_size()
    }

    fn property_size(&self) -> u32 {
        self.reason_desc
            .as_ref()
            .map_or(0, |d| PROP_SIZE_UTF8_STRING + d.len() as u32)
            + self.user_props.as_ref().map_or(0, |u| u.property_size())
    }

    fn payload_size(&self) -> u32 {
        self.sub_reason.len() as u32
    }
}

impl Decode for SubAck {
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MQTTCodecError> {
        if src.remaining() < 4 {
            return Err(MQTTCodecError::new(
                "MQTTv5 3.9.3 insufficient data for SUBACK",
            ));
        }
        self.packet_id = src.get_u16();
        let prop_len = decode_variable_len_integer(src);
        if src.remaining() < prop_len as usize {
            return Err(MQTTCodecError::new(
                "MQTTv5 3.9.3 insufficient data for SUBACK properties",
            ));
        }
        let read_until = src.remaining() - prop_len as usize;
        let mut reason_id_prop: bool = false;
        while src.remaining() > read_until {
            match PropertyType::try_from(src.get_u8()) {
                Ok(property_type) => {
                    if property_type != PropertyType::UserProperty {
                        match property_type {
                            PropertyType::Reason => {
                                if reason_id_prop {
                                    return Err(MQTTCodecError::new(
                                        "MQTTv5 3.9.2.1.2 reason description can appear only once",
                                    ));
                                }
                                self.reason_desc = Some(decode_utf8_string(src)?);
                                reason_id_prop = true;
                            }
                            _ => {
                                return Err(MQTTCodecError::new(
                                    "MQTTv5 3.8.2.1 unexpected property",
                                ))
                            }
                        }
                    } else {
                        if self.user_props.is_none() {
                            self.user_props = Some(UserPropertyMap::new());
                        }
                        let property_map = self.user_props.as_mut().unwrap();
                        let key = decode_utf8_string(src)?;
                        let value = decode_utf8_string(src)?;
                        property_map.add_property(&key, &value);
                    }
                }
                Err(_) => todo!(),
            }
        }
        Ok(())
    }
}

impl Encode for SubAck {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
        todo!()
    }
}

/// Subscription represents an MQTT v5 3.8.3 SUBSCRIBE Payload. The MQTT v5
/// 3.8.3.1 options are represented as individual fields in the struct.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Subscription {
    pub filter: String,
    pub qos: QoSLevel,
    pub no_local: bool,
    pub retain_as: bool,
    pub handling: RetainHandling,
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
    pub sub_id: Option<u32>,
    pub user_props: Option<UserPropertyMap>,
    payload: Vec<Subscription>,
}

impl Subscribe {

    pub fn new(packet_id: u16, sub_id: Option<u32>, payload: Vec<Subscription>) -> Self {
        Self{
            packet_id,
            sub_id,
            user_props: None,
            payload
        }
    }

    pub fn packet_id(&self) -> u16 {
        self.packet_id
    }

    pub fn set_packet_id(&mut self, packet_id: u16) {
        // TODO replace with codec error
        assert!(packet_id != 0);
        self.packet_id = packet_id;
    }

    pub fn add_subscription(&mut self, subscription: Subscription) {
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
        self.sub_id
            .map_or(0, |id| PROP_ID_LEN + variable_byte_int_size(id))
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
        encode_variable_len_integer(self.property_size(), dest);
        if let Some(sub_id) = self.sub_id {
            encode_var_int_property(PropertyType::SubscriptionId, sub_id, dest);
        }
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
                    } else {
                        if self.user_props.is_none() {
                            self.user_props = Some(UserPropertyMap::new());
                        }
                        let property_map = self.user_props.as_mut().unwrap();
                        let key = decode_utf8_string(src)?;
                        let value = decode_utf8_string(src)?;
                        property_map.add_property(&key, &value);
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

    use crate::{Decode, Encode, QoSLevel, Size, Subscribe, UserPropertyMap};

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
    fn test_encode_properties() {
        const USER_PROP_KEY: &str = "btf_mgmt";
        const USER_PROP_VALUE: &str = "management";
        const USER_PROP_SIZE: usize = 5 + USER_PROP_KEY.len() + USER_PROP_VALUE.len();
        // USER PROPS + SUB ID PROP
        const EXPECTED_PROP_SIZE: u32 = USER_PROP_SIZE as u32 + 3;
        const EXPECTED_PAYLOAD_SIZE: u32 = 7;
        const EXPECTED_SIZE: u32 = 5 + EXPECTED_PAYLOAD_SIZE + EXPECTED_PROP_SIZE;
        let mut subscribe = Subscribe::default();
        subscribe.packet_id = 101;
        let mut usr = UserPropertyMap::new();
        usr.add_property(USER_PROP_KEY, USER_PROP_VALUE);
        subscribe.user_props = Some(usr);
        subscribe.sub_id = Some(4096);
        let subscription = Subscription {
            filter: "test".to_string(),
            qos: QoSLevel::AtLeastOnce,
            retain_as: false,
            no_local: false,
            handling: RetainHandling::None,
        };
        subscribe.add_subscription(subscription);
        assert_eq!(EXPECTED_PROP_SIZE, subscribe.property_size());
        assert_eq!(EXPECTED_PAYLOAD_SIZE, subscribe.payload_size());
        let mut dest = BytesMut::new();
        match subscribe.encode(&mut dest) {
            Ok(()) => {
                assert_eq!(EXPECTED_SIZE, dest.len() as u32);
            }
            Err(e) => panic!("Unexpected encoding error: {}", e.reason),
        }
    }

    #[test]
    #[rustfmt::skip]
    fn test_basic_decode() {
        const ENCODED_PACKET: [u8; 43] = [
            0xba,
            0xba, // packet id
            0x02, // property length
            0x0b,
            0x0A, // subscription id
            0x00, 0x23, 0x2f, 0x66, 0x75, 0x73, 0x69, 0x6f, 0x6e, 
            0x2f, 0x75, 0x70, 0x64, 0x61, 0x74, 0x65, 0x2f, 0x66,
            0x72, 0x69, 0x73, 0x63, 0x6f, 0x5f, 0x30, 0x31, 0x2f, 
            0x73, 0x65, 0x6e, 0x73, 0x6f, 0x72, 0x5f, 0x30, 0x31, 
            0x33,
            0b_0001_1101, // subscription flags
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
