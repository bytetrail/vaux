use crate::{
    codec::{self, encode_string},
    MqttCodecError,
};
use bytes::{BufMut, BytesMut};
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
};

/// MQTT property type. For more information on the specific property types,
/// please see the
/// [MQTT Specification](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901027).
/// All of the types below are MQTT protocol types.
/// Identifier | Name | Type
/// -----------+------+-----
/// 0x01 | Payload Format Indicator | byte
/// 0x02 | Message Expiry Interval | 4 byte Integer
/// 0x03 | Content Type | UTF-8 string
/// 0x08 | Response Topic | UTF-8 string
/// 0x09 | Correlation Data | binary data
/// 0x0b | Subscription Identifier | Variable Length Integer
/// 0x11 | Session Expiry Interval | 4 byte Integer
/// 0x12 | Assigned Client Identifier | UTF-8 string
/// 0x13 | Server Keep Alive | 2 byte integer
/// 0x15 | Authentication Method | UTF-8 string
/// 0x16 | Authentication Data | binary data
/// 0x17 | Request Problem Information | byte
/// 0x18 | Will Delay Interval | 4 byte integer
/// 0x19 | Request Response Information | byte
/// 0x1a | Response Information | UTF-8 string
/// 0x1c | Server Reference | UTF-8 string
/// 0x1f | Reason String | UTF-8 string
/// 0x21 | Receive Maximum | 2 byte integer
/// 0x22 | Topic Alias Maximum | 2 byte integer
/// 0x23 | Topic Alias | 2 byte integer
/// 0x24 | Maximum QoS | byte
/// 0x25 | Retain Available | byte
/// 0x26 | User Property | UTF-8 string pair
/// 0x27 | Maximum Packet Size | 4 byte integer
/// 0x28 | Wildcard Subscription Available | byte
/// 0x29 | Subscription Identifier Available | byte
/// 0x2a | Shared Subscription Available | byte
#[repr(u8)]
#[derive(Hash, Debug, Copy, Clone, PartialEq, Eq)]
pub enum PropertyType {
    PayloadFormat = 0x01,
    MessageExpiry = 0x02,
    ContentType = 0x03,
    ResponseTopic = 0x08,
    CorrelationData = 0x09,
    SubscriptionIdentifier = 0x0b,
    SessionExpiryInterval = 0x11,
    AssignedClientId = 0x12,
    KeepAlive = 0x13,
    AuthMethod = 0x15,
    AuthData = 0x16,
    ReqProblemInfo = 0x17,
    WillDelay = 0x18,
    ReqRespInfo = 0x19,
    RespInfo = 0x1a,
    ServerReference = 0x1c,
    ReasonString = 0x1f,
    RecvMax = 0x21,
    TopicAliasMax = 0x22,
    TopicAlias = 0x23,
    MaxQoS = 0x24,
    RetainAvail = 0x25,
    UserProperty = 0x26,
    MaxPacketSize = 0x27,
    WildcardSubAvail = 0x28,
    SubIdAvail = 0x29,
    ShardSubAvail = 0x2a,
}

impl Display for PropertyType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PropertyType::PayloadFormat => write!(f, "\"Payload Format Indicator\""),
            PropertyType::MessageExpiry => write!(f, "\"Message Expiry Interval\""),
            PropertyType::ContentType => write!(f, "\"Content Type\""),
            PropertyType::ResponseTopic => write!(f, "\"Response Topic\""),
            PropertyType::CorrelationData => write!(f, "\"Correlation Data\""),
            PropertyType::SubscriptionIdentifier => write!(f, "\"Subscription Identifier\""),
            PropertyType::SessionExpiryInterval => write!(f, "\"Session Expiry Interval\""),
            PropertyType::AssignedClientId => write!(f, "\"Assigned Client Identifier\""),
            PropertyType::KeepAlive => write!(f, "\"Server Keep Alive\""),
            PropertyType::AuthMethod => write!(f, "\"Authentication Method\""),
            PropertyType::AuthData => write!(f, "\"Authentication Data\""),
            PropertyType::ReqProblemInfo => write!(f, "\"Request Problem Information\""),
            PropertyType::WillDelay => write!(f, "\"Will Delay Interval\""),
            PropertyType::ReqRespInfo => write!(f, "\"Request Response Information\""),
            PropertyType::RespInfo => write!(f, "\"Response Information\""),
            PropertyType::ServerReference => write!(f, "\"Server Reference\""),
            PropertyType::ReasonString => write!(f, "\"Reason String\""),
            PropertyType::RecvMax => write!(f, "\"Receive Maximum\""),
            PropertyType::TopicAliasMax => write!(f, "\"Topic Alias Maximum\""),
            PropertyType::TopicAlias => write!(f, "\"Topic Alias\""),
            PropertyType::MaxQoS => write!(f, "\"Maximum QoS\""),
            PropertyType::RetainAvail => write!(f, "\"Retain Available\""),
            PropertyType::UserProperty => write!(f, "\"User Property\""),
            PropertyType::MaxPacketSize => write!(f, "\"Maximum Packet Size\""),
            PropertyType::WildcardSubAvail => write!(f, "\"Wildcard Substitution Available\""),
            PropertyType::SubIdAvail => write!(f, "\"Subscription Identifier Available\""),
            PropertyType::ShardSubAvail => write!(f, "\"Shared Subscription Available\""),
        }
    }
}

impl TryFrom<u8> for PropertyType {
    type Error = MqttCodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(PropertyType::PayloadFormat),
            0x02 => Ok(PropertyType::MessageExpiry),
            0x03 => Ok(PropertyType::ContentType),
            0x08 => Ok(PropertyType::ResponseTopic),
            0x09 => Ok(PropertyType::CorrelationData),
            0x0b => Ok(PropertyType::SubscriptionIdentifier),
            0x11 => Ok(PropertyType::SessionExpiryInterval),
            0x12 => Ok(PropertyType::AssignedClientId),
            0x13 => Ok(PropertyType::KeepAlive),
            0x15 => Ok(PropertyType::AuthMethod),
            0x16 => Ok(PropertyType::AuthData),
            0x17 => Ok(PropertyType::ReqProblemInfo),
            0x18 => Ok(PropertyType::WillDelay),
            0x19 => Ok(PropertyType::ReqRespInfo),
            0x1a => Ok(PropertyType::RespInfo),
            0x1c => Ok(PropertyType::ServerReference),
            0x1f => Ok(PropertyType::ReasonString),
            0x21 => Ok(PropertyType::RecvMax),
            0x22 => Ok(PropertyType::TopicAliasMax),
            0x23 => Ok(PropertyType::TopicAlias),
            0x24 => Ok(PropertyType::MaxQoS),
            0x25 => Ok(PropertyType::RetainAvail),
            0x26 => Ok(PropertyType::UserProperty),
            0x27 => Ok(PropertyType::MaxPacketSize),
            0x28 => Ok(PropertyType::WildcardSubAvail),
            0x29 => Ok(PropertyType::SubIdAvail),
            0x2a => Ok(PropertyType::ShardSubAvail),
            p => Err(MqttCodecError::new(&format!(
                "MQTTv5 2.2.2.2 invalid property type identifier: {p}"
            ))),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UserProperty(HashMap<String, Vec<String>>);

impl codec::PropertyCodecSize for UserProperty {
    fn property_size(&self) -> u32 {
        let mut size = 0;
        for (key, values) in &self.0 {
            for value in values {
                size += 1; // Property identifier
                size += 2 + key.len() as u32; // Key length + key
                size += 2 + value.len() as u32; // Value length + value
            }
        }
        size
    }
}

impl codec::Encode for UserProperty {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        for (key, values) in &self.0 {
            for value in values {
                dest.put_u8(PropertyType::UserProperty as u8);
                encode_string(key, dest)?;
                encode_string(value, dest)?;
            }
        }
        Ok(())
    }
}

impl codec::Decode for UserProperty {
    /// Decode a single user property key-value pair and add it to the map.
    fn decode(&mut self, src: &mut BytesMut) -> Result<usize, MqttCodecError> {
        let (key_value, key_len) = codec::decode_string(src)?;
        let (value, len) = codec::decode_string(src)?;
        let bytes_read = 2 + key_len + 2 + len;
        self.0.entry(key_value).or_insert_with(Vec::new).push(value);
        Ok(bytes_read)
    }
}

impl UserProperty {
    pub fn new() -> Self {
        UserProperty(HashMap::new())
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn clear_codec(&mut self, key: &str) {
        self.0.remove(key);
    }

    pub fn add(&mut self, key: String, value: String) {
        self.0.entry(key).or_insert_with(Vec::new).push(value);
    }

    pub fn get(&self, key: &str) -> Option<&Vec<String>> {
        self.0.get(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
        self.0.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}
