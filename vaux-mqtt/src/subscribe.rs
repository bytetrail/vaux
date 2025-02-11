use std::collections::HashSet;

use bytes::{Buf, BufMut, BytesMut};

use crate::{
    codec::{get_utf8, put_utf8, variable_byte_int_size},
    property::{PacketProperties, Property, PropertyBundle},
    Decode, Encode, FixedHeader, MqttCodecError, MqttError, MqttVersion, PropertyType, QoSLevel,
    Reason, Size,
};

lazy_static! {
    static ref SUBACK_SUPPORTED: HashSet<PropertyType> = {
        let mut supported = HashSet::new();
        supported.insert(PropertyType::ReasonString);
        supported.insert(PropertyType::UserProperty);
        supported
    };
    static ref SUBSCRIPTION_SUPPORTED: HashSet<PropertyType> = {
        let mut supported = HashSet::new();
        supported.insert(PropertyType::SubscriptionIdentifier);
        supported.insert(PropertyType::UserProperty);
        supported
    };
}

const VAR_HDR_LEN: u32 = 2;
const MIN_SUBSCRIPTION_ID: u32 = 1;
const MAX_SUBSCRIPTION_ID: u32 = 268_435_455;

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
    type Error = MqttCodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(RetainHandling::Send),
            0x01 => Ok(RetainHandling::SendNew),
            0x02 => Ok(RetainHandling::None),
            v => Err(MqttCodecError::new(
                format!("Mqttv5 3.8.3.1 invalid retain option: {}", v).as_str(),
            )),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SubAck {
    packet_id: u16,
    props: PropertyBundle,
    sub_reason: Vec<Reason>,
}

impl SubAck {
    pub fn packet_id(&self) -> u16 {
        self.packet_id
    }

    pub fn properties(&self) -> &PropertyBundle {
        &self.props
    }

    pub fn reason(&self) -> &Vec<Reason> {
        &self.sub_reason
    }
}

impl Size for SubAck {
    fn size(&self) -> u32 {
        let prop_size = self.property_size();
        VAR_HDR_LEN + variable_byte_int_size(prop_size) + prop_size + self.payload_size()
    }

    fn property_size(&self) -> u32 {
        self.props.size()
    }

    fn payload_size(&self) -> u32 {
        self.sub_reason.len() as u32
    }
}

impl Decode for SubAck {
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MqttCodecError> {
        if src.remaining() < 4 {
            return Err(MqttCodecError::new(
                "MQTTv5 3.9.3 insufficient data for SUBACK",
            ));
        }
        self.packet_id = src.get_u16();
        self.props.decode(src)?;
        // decode the reason codes
        while src.has_remaining() {
            let reason = Reason::try_from(src.get_u8())?;
            self.sub_reason.push(reason);
        }
        Ok(())
    }
}

impl Encode for SubAck {
    fn encode(&self, _dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        todo!()
    }
}

/// Subscription represents an MQTT v5 3.8.3 SUBSCRIBE Payload. The Mqtt v5
/// 3.8.3.1 options are represented as individual fields in the struct.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SubscriptionFilter {
    pub filter: String,
    pub qos: QoSLevel,
    pub no_local: bool,
    pub retain_as: bool,
    pub handling: RetainHandling,
}

impl SubscriptionFilter {
    /// Create a new Subscription with the given filter and QoSLevel.
    /// The remaining fields are set to their default values.
    pub fn new(filter: String, qos: QoSLevel) -> Self {
        Self {
            filter,
            qos,
            ..Default::default()
        }
    }

    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        put_utf8(&self.filter, dest)?;
        let flags = self.qos as u8
            | (self.no_local as u8) << 2
            | (self.retain_as as u8) << 3
            | (self.handling as u8) << 4;
        dest.put_u8(flags);
        Ok(())
    }

    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MqttCodecError> {
        self.filter = get_utf8(src)?;
        let flags = src.get_u8();
        self.qos = QoSLevel::try_from(flags & 0b_0000_0011)?;
        self.no_local = flags & 0b_0000_0100 == 0b_0000_0100;
        self.retain_as = flags & 0b_0000_1000 == 0b_0000_1000;
        self.handling = RetainHandling::try_from(flags & 0b_0011_0000 >> 4)?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Subscription {
    id: Option<u32>,
    pub filter: Vec<SubscriptionFilter>,
}

impl Subscription {
    pub fn new(id: u32, filter: Vec<SubscriptionFilter>) -> Result<Self, MqttError> {
        let mut s = Self { id: None, filter };
        s.set_id(id)?;
        Ok(s)
    }

    pub fn id(&self) -> Option<u32> {
        self.id
    }

    pub fn set_id(&mut self, id: u32) -> Result<(), MqttError> {
        if id < MIN_SUBSCRIPTION_ID || id > MAX_SUBSCRIPTION_ID {
            return Err(MqttError::new_from_spec(
                MqttVersion::V5,
                "3.8.3",
                "subscription ID must be between 1 and 268,435,455",
            ));
        }
        self.id = Some(id);
        Ok(())
    }

    fn id_from_properties(props: &PropertyBundle) -> Option<u32> {
        if let Some(sub_id_prop) = props.get_property(PropertyType::SubscriptionIdentifier) {
            if let Property::SubscriptionIdentifier(id) = sub_id_prop {
                return Some(*id);
            }
        }
        None
    }
}

impl From<&Subscribe> for Subscription {
    fn from(sub: &Subscribe) -> Self {
        let mut filter = Vec::new();
        for s in &sub.payload {
            filter.push(s.clone());
        }

        Self {
            id: Self::id_from_properties(sub.properties()),
            filter,
        }
    }
}

impl From<Subscribe> for Subscription {
    fn from(sub: Subscribe) -> Self {
        let mut filter = Vec::new();
        let id = Self::id_from_properties(sub.properties());
        for s in sub.payload {
            filter.push(s);
        }
        Self { id, filter }
    }
}

/// Create a Subscribe packet from a Subscription. The SUBSCRIBE packet
/// is created with the Subscription Identifier property set to the id
/// of the Subscription when that ID is Some.
impl From<&Subscription> for Subscribe {
    fn from(sub: &Subscription) -> Self {
        let mut subscribe = Subscribe::default();
        if let Some(id) = sub.id {
            subscribe
                .props
                .set_property(Property::SubscriptionIdentifier(id));
        }
        for s in &sub.filter {
            subscribe.add_subscription(s.clone());
        }
        subscribe
    }
}

impl From<Subscription> for Subscribe {
    fn from(sub: Subscription) -> Self {
        let mut subscribe = Subscribe::default();
        if let Some(id) = sub.id {
            subscribe
                .props
                .set_property(Property::SubscriptionIdentifier(id));
        }
        for s in sub.filter {
            subscribe.add_subscription(s);
        }
        subscribe
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscribe {
    packet_id: u16,
    props: PropertyBundle,
    payload: Vec<SubscriptionFilter>,
}

impl Default for Subscribe {
    fn default() -> Self {
        Self {
            packet_id: 0,
            props: PropertyBundle::new(SUBSCRIPTION_SUPPORTED.clone()),
            payload: Vec::new(),
        }
    }
}

impl Subscribe {
    pub fn new(packet_id: u16, payload: Vec<SubscriptionFilter>) -> Self {
        Self {
            packet_id,
            props: PropertyBundle::new(SUBSCRIPTION_SUPPORTED.clone()),
            payload,
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

    pub fn add_subscription(&mut self, subscription: SubscriptionFilter) {
        self.payload.push(subscription);
    }

    fn encode_payload(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        if self.payload.is_empty() {
            return Err(MqttCodecError::new(
                "MQTTv5 3.8.3 subscribe payload must exist",
            ));
        }
        for sub in &self.payload {
            sub.encode(dest)?;
        }
        Ok(())
    }
}

impl PacketProperties for Subscribe {
    fn properties(&self) -> &PropertyBundle {
        &self.props
    }

    fn properties_mut(&mut self) -> &mut PropertyBundle {
        &mut self.props
    }

    fn set_properties(&mut self, props: PropertyBundle) {
        self.props = props;
    }
}

impl Size for Subscribe {
    fn size(&self) -> u32 {
        let prop_size = self.property_size();
        VAR_HDR_LEN + prop_size + variable_byte_int_size(prop_size) + self.payload_size()
    }

    fn property_size(&self) -> u32 {
        self.props.size()
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
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        if self.packet_id == 0 {
            return Err(MqttCodecError::new(
                "MQTTv5 2.2.1 packet identifier must not be 0",
            ));
        }
        let mut hdr = FixedHeader::new(crate::PacketType::Subscribe);
        hdr.set_remaining(self.size());
        hdr.encode(dest)?;
        dest.put_u16(self.packet_id);
        self.props.encode(dest)?;
        self.encode_payload(dest)?;
        Ok(())
    }
}

impl Decode for Subscribe {
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MqttCodecError> {
        if src.len() < 3 {
            return Err(MqttCodecError::new(
                "MQTTv5 3.8.2 insufficient data for SUBSCRIBE",
            ));
        }
        self.packet_id = src.get_u16();
        self.props.decode(src)?;
        while src.remaining() != 0 {
            let mut s = SubscriptionFilter::default();
            s.decode(src)?;
            self.add_subscription(s);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use bytes::BytesMut;

    use crate::{
        property::{PacketProperties, Property},
        Decode, Encode, QoSLevel, Size, Subscribe,
    };

    use super::{RetainHandling, SubscriptionFilter};

    #[test]
    fn test_encode_flags() {
        const EXPECTED_FLAG: u8 = 0b_0010_1110;
        const EXPECTED_LEN: usize = 7;
        let mut sub = SubscriptionFilter::default();
        sub.filter = "test".to_string();
        sub.qos = QoSLevel::ExactlyOnce;
        sub.no_local = true;
        sub.retain_as = true;
        sub.handling = RetainHandling::None;

        let mut dest = BytesMut::new();
        assert!(sub.encode(&mut dest).is_ok());
        assert_eq!(EXPECTED_LEN, dest.len());
        assert_eq!(EXPECTED_FLAG, dest[6]);

        const SEND_EXPECTED_FLAG: u8 = 0b_0001_1110;
        sub.handling = RetainHandling::SendNew;
        let mut dest = BytesMut::new();
        assert!(sub.encode(&mut dest).is_ok());
        assert_eq!(EXPECTED_LEN, dest.len());
        assert_eq!(SEND_EXPECTED_FLAG, dest[6]);

        const QOS_EXPECTED_FLAG: u8 = 0b_0000_1101;
        sub.qos = QoSLevel::AtLeastOnce;
        sub.handling = RetainHandling::Send;
        let mut dest = BytesMut::new();

        assert!(sub.encode(&mut dest).is_ok());
        assert_eq!(EXPECTED_LEN, dest.len());
        assert_eq!(QOS_EXPECTED_FLAG, dest[6]);
    }

    #[test]
    fn test_payload_size() {
        const EXPECTED_PAYLOAD_SIZE: u32 = 7;

        let mut subscribe = Subscribe::default();
        subscribe.packet_id = 42;
        let subscription = SubscriptionFilter {
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
        let subscription = SubscriptionFilter {
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
        subscribe.packet_id = 42;
        let props = subscribe.properties_mut();
        props.add_user_property(USER_PROP_KEY.to_string(), USER_PROP_VALUE.to_string());
        props.set_property(Property::SubscriptionIdentifier(4096));
        let subscription = SubscriptionFilter {
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
