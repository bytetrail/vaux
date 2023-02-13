use std::fmt::{Display, Formatter};

use bytes::{BytesMut, Buf};

use crate::{MQTTCodecError, QoSLevel, codec::{get_utf8, get_var_u32, get_bin, get_bool}};

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
    SubscriptionId = 0x0b,
    SessionExpiryInt = 0x11,
    AssignedClientId = 0x12,
    KeepAlive = 0x13,
    AuthMethod = 0x15,
    AuthData = 0x16,
    ReqProblemInfo = 0x17,
    WillDelay = 0x18,
    ReqRespInfo = 0x19,
    RespInfo = 0x1a,
    ServerRef = 0x1c,
    Reason = 0x1f,
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
            PropertyType::SubscriptionId => write!(f, "\"Subscription Identifier\""),
            PropertyType::SessionExpiryInt => write!(f, "\"Session Expiry Interval\""),
            PropertyType::AssignedClientId => write!(f, "\"Assigned Client Identifier\""),
            PropertyType::KeepAlive => write!(f, "\"Server Keep Alive\""),
            PropertyType::AuthMethod => write!(f, "\"Authentication Method\""),
            PropertyType::AuthData => write!(f, "\"Authentication Data\""),
            PropertyType::ReqProblemInfo => write!(f, "\"Request Problem Information\""),
            PropertyType::WillDelay => write!(f, "\"Will Delay Interval\""),
            PropertyType::ReqRespInfo => write!(f, "\"Request Response Information\""),
            PropertyType::RespInfo => write!(f, "\"Response Information\""),
            PropertyType::ServerRef => write!(f, "\"Server Reference\""),
            PropertyType::Reason => write!(f, "\"Reason String\""),
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
    type Error = MQTTCodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(PropertyType::PayloadFormat),
            0x02 => Ok(PropertyType::MessageExpiry),
            0x03 => Ok(PropertyType::ContentType),
            0x08 => Ok(PropertyType::ResponseTopic),
            0x09 => Ok(PropertyType::CorrelationData),
            0x0b => Ok(PropertyType::SubscriptionId),
            0x11 => Ok(PropertyType::SessionExpiryInt),
            0x12 => Ok(PropertyType::AssignedClientId),
            0x13 => Ok(PropertyType::KeepAlive),
            0x15 => Ok(PropertyType::AuthMethod),
            0x16 => Ok(PropertyType::AuthData),
            0x17 => Ok(PropertyType::ReqProblemInfo),
            0x18 => Ok(PropertyType::WillDelay),
            0x19 => Ok(PropertyType::ReqRespInfo),
            0x1a => Ok(PropertyType::RespInfo),
            0x1c => Ok(PropertyType::ServerRef),
            0x1f => Ok(PropertyType::Reason),
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
            p => Err(MQTTCodecError::new(&format!(
                "MQTTv5 2.2.2.2 invalid property type identifier: {}",
                p
            ))),
        }
    }
}

#[repr(u8)]
pub enum PayloadFormat {
    Bin = 0x00,
    Utf8 = 0x01,
}

impl TryFrom<u8> for PayloadFormat {
    type Error = MQTTCodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(PayloadFormat::Bin),
            0x01 => Ok(PayloadFormat::Utf8),
            _ => Err(MQTTCodecError::new("invalid payload format")),
        }
    }
}

#[repr(u8)]
pub enum Property {
    PayloadFormat(PayloadFormat) = 0x01,
    MessageExpiry(u32) = 0x02,
    ContentType(String) = 0x03,
    ResponseTopic(String) = 0x08,
    CorrelationData(Vec<u8>) = 0x09,
    SubscriptionId(u32) = 0x0b,
    SessionExpiryInt(u32) = 0x11,
    AssignedClientId(String) = 0x12,
    KeepAlive(u16) = 0x13,
    AuthMethod(String) = 0x15,
    AuthData(Vec<u8>) = 0x16,
    ReqProblemInfo(u8) = 0x17,
    WillDelay(u32) = 0x18,
    ReqRespInfo(u8) = 0x19,
    RespInfo(String) = 0x1a,
    ServerRef(String) = 0x1c,
    Reason(String) = 0x1f,
    RecvMax(u16) = 0x21,
    TopicAliasMax(u16) = 0x22,
    TopicAlias(u16) = 0x23,
    MaxQoS(QoSLevel) = 0x24,
    RetainAvail(bool) = 0x25,
    UserProperty(String, String) = 0x26,
    MaxPacketSize(u32) = 0x27,
    WildcardSubAvail(bool) = 0x28,
    SubIdAvail(bool) = 0x29,
    ShardSubAvail(bool) = 0x2a,
}

impl Property {
    //pub fn decode_all(map: &mut PropertyMap, src: &mut BytesMut) {}

    pub fn decode(src: &mut BytesMut) -> Result<Property, MQTTCodecError> {
        match PropertyType::try_from(src.get_u8()) {
            Ok(prop_type) => match prop_type {
                PropertyType::PayloadFormat => Ok(Property::PayloadFormat(
                    PayloadFormat::try_from(src.get_u8())?,
                )),
                PropertyType::MessageExpiry => Ok(Property::MessageExpiry(src.get_u32())),
                PropertyType::ContentType => Ok(Property::ContentType(get_utf8(src)?)),
                PropertyType::ResponseTopic => Ok(Property::ResponseTopic(get_utf8(src)?)),
                PropertyType::CorrelationData => Ok(Property::CorrelationData(get_bin(src)?)),
                PropertyType::SubscriptionId => Ok(Property::SubscriptionId(get_var_u32(src))),
                PropertyType::SessionExpiryInt => Ok(Property::SessionExpiryInt(src.get_u32())),
                PropertyType::AssignedClientId => Ok(Property::AssignedClientId(get_utf8(src)?)),
                PropertyType::KeepAlive => Ok(Property::KeepAlive(src.get_u16())),
                PropertyType::AuthMethod => Ok(Property::AuthMethod(get_utf8(src)?)),
                PropertyType::AuthData => Ok(Property::AuthData(get_bin(src)?)),
                PropertyType::ReqProblemInfo => Ok(Property::ReqProblemInfo(src.get_u8())),
                PropertyType::WillDelay => Ok(Property::WillDelay(src.get_u32())),
                PropertyType::ReqRespInfo => Ok(Property::ReqRespInfo(src.get_u8())),
                PropertyType::RespInfo => Ok(Property::RespInfo(get_utf8(src)?)),
                PropertyType::ServerRef => Ok(Property::ServerRef(get_utf8(src)?)),
                PropertyType::Reason => Ok(Property::Reason(get_utf8(src)?)),
                PropertyType::RecvMax => Ok(Property::RecvMax(src.get_u16())),
                PropertyType::TopicAliasMax => Ok(Property::TopicAliasMax(src.get_u16())),
                PropertyType::TopicAlias => Ok(Property::TopicAlias(src.get_u16())),
                PropertyType::MaxQoS => Ok(Property::MaxQoS(QoSLevel::try_from(src.get_u8())?)),
                PropertyType::RetainAvail => Ok(Property::RetainAvail(get_bool(src)?)),
                PropertyType::UserProperty => {
                    let k = get_utf8(src)?;
                    let v = get_utf8(src)?;
                    Ok(Property::UserProperty(k, v))
                }
                PropertyType::MaxPacketSize => Ok(Property::MaxPacketSize(src.get_u32())),
                PropertyType::WildcardSubAvail => Ok(Property::WildcardSubAvail(get_bool(src)?)),
                PropertyType::SubIdAvail => Ok(Property::SubIdAvail(get_bool(src)?)),
                PropertyType::ShardSubAvail => Ok(Property::ShardSubAvail(get_bool(src)?)),
            },
            Err(_) => todo!(),
        }
    }
}

pub trait PropertyEncode {
    fn property_encode() -> Result<(), MQTTCodecError>;
}

pub trait PropertySize {
    fn property_size_internal() -> u32;
}
