use std::{fmt::{Display, Formatter}, collections::{HashSet, HashMap}, ops::Index};

use bytes::{Buf, BytesMut, BufMut};

use crate::{
    codec::{get_bin, get_bool, get_utf8, get_var_u32, encode_u8_property, encode_utf8_property, encode_u32_property, encode_u16_property, encode_bool_property, encode_bin_property, put_utf8, encode_var_int_property, variable_byte_int_size, put_var_u32},
    MqttCodecError, QoSLevel, Decode, Encode, Size,
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
    type Error = MqttCodecError;

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
            p => Err(MqttCodecError::new(&format!(
                "MQTTv5 2.2.2.2 invalid property type identifier: {}",
                p
            ))),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PropertyBundle {
    supported: HashSet<PropertyType>,
    properties: HashMap<PropertyType, Property>,
    user_props: HashMap<String, Vec<String>>,
}

impl PropertyBundle {

    pub(crate) fn new(supported: HashSet<PropertyType>) -> Self {
        Self {
            supported,
            properties: HashMap::new(),
            user_props: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        let mut len= self.properties.len();
        for item in &self.user_props {
            len += item.1.len();
        }
        len
    }

    pub fn supports_property(&self, prop_type: &PropertyType) -> bool {
        self.supported.contains(prop_type)
    }

    pub fn has_property(&self, prop_type: &PropertyType) -> bool {
        self.properties.contains_key(prop_type)
    }

    pub fn get_property(&self, prop_type: &PropertyType) -> Option<&Property> {
        self.properties.get(prop_type)
    }

    pub fn set_property(&mut self, prop: Property) {
        if let Property::UserProperty(key, value) = prop {
            self.add_user_property(key, value);
        } else {
            self.properties.insert((&prop).into(), prop);
        }
    }

    pub fn add_user_property(&mut self, key: String, value: String) {
        if self.user_props.contains_key(&key) {
            self.user_props.get_mut(&key).unwrap().push(value)
        } else {
            let value = vec![value];
            self.user_props.insert(key, value);
        }
    }   
}

impl Index<PropertyType> for PropertyBundle {
    type Output = Property;

    fn index(&self, prop_type: PropertyType) -> &Self::Output {
        &self.properties[&prop_type.into()]
    }
}


impl Size for PropertyBundle {
    fn size(&self) -> u32 {
        let mut size = 0_u32;
        for (_, prop) in &self.properties {
            match prop {
                Property::ContentType(p) | 
                Property::ResponseTopic(p) |
                Property::AssignedClientId(p) |
                Property::AuthMethod(p) |
                Property::RespInfo(p) |
                Property::ServerRef(p) |
                Property::Reason(p) => size += p.len() as u32 + 3,
    
                Property::SubscriptionId(p) => size += variable_byte_int_size(*p) + 1,
    
                Property::MessageExpiry(_) |
                Property::SessionExpiryInt(_) |
                Property::WillDelay(_) |
                Property::MaxPacketSize(_) => size += 5,
            
                Property::KeepAlive(_) |
                Property::RecvMax(_) |
                Property::TopicAliasMax(_) |
                Property::TopicAlias(_) => size += 3,
            
                Property::PayloadFormat(_) =>  size += 2,
    
                Property::ReqProblemInfo(_) |
                Property::ReqRespInfo(_) => size += 2,
                Property::MaxQoS(_) => size += 2,
    
                Property::RetainAvail(_) |
                Property::WildcardSubAvail(_) |
                Property::SubIdAvail(_) |
                Property::ShardSubAvail(_) => size += 2,
    
                Property::CorrelationData(p) |
                Property::AuthData(p) => size += p.len() as u32 + 3,   
                // ignore user properties here
                _ => {},             
            }
        }

        for (key, values) in &self.user_props {
            for value in values {
                size += value.len() as u32 + 2;

            }
            size += key.len() as u32 + 3;
        }
        size        
    }

    fn property_size(&self) -> u32 {
        unimplemented!()
    }

    fn payload_size(&self) -> u32 {
        unimplemented!()
    }
}

impl Encode for PropertyBundle {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        put_var_u32(self.size(), dest);
        for (_, prop) in &self.properties {
            prop.encode(dest)?;
        }
        for (key, values) in &self.user_props {
            for value in values {
                Property::UserProperty(key.clone(), value.clone()).encode(dest)?;
            }
        }
        Ok(())
    }
}

impl Decode for PropertyBundle {
    fn decode(&mut self, src: &mut BytesMut) -> Result<(), MqttCodecError> {
        let prop_size = get_var_u32(src) as usize;
        let remaining = src.remaining();
        let prop_remaining = remaining - prop_size;
        while src.remaining() > prop_remaining {
            self.set_property(Property::decode(src)?);
            println!("remaining: {}", src.remaining());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PayloadFormat {
    Bin = 0x00,
    Utf8 = 0x01,
}

impl TryFrom<u8> for PayloadFormat {
    type Error = MqttCodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(PayloadFormat::Bin),
            0x01 => Ok(PayloadFormat::Utf8),
            _ => Err(MqttCodecError::new("invalid payload format")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub fn decode(src: &mut BytesMut) -> Result<Property, MqttCodecError> {
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

    pub fn encode(&self, dest: &mut bytes::BytesMut) -> Result<(), MqttCodecError> {
        match self {
            Property::ContentType(p) | 
            Property::ResponseTopic(p) |
            Property::AssignedClientId(p) |
            Property::AuthMethod(p) |
            Property::RespInfo(p) |
            Property::ServerRef(p) |
            Property::Reason(p) => encode_utf8_property(self.into(), p, dest)?,

            Property::SubscriptionId(p) => encode_var_int_property(self.into(), *p, dest),

            Property::MessageExpiry(p) |
            Property::SessionExpiryInt(p) |
            Property::WillDelay(p) |
            Property::MaxPacketSize(p) => encode_u32_property(self.into(), *p, dest),
        
            Property::KeepAlive(p) |
            Property::RecvMax(p) |
            Property::TopicAliasMax(p) |
            Property::TopicAlias(p) => encode_u16_property(self.into(), *p, dest),
        
            Property::PayloadFormat(p) =>  encode_u8_property(self.into(), *p as u8, dest),

            Property::ReqProblemInfo(p) |
            Property::ReqRespInfo(p) => encode_u8_property(self.into(), *p, dest),
            Property::MaxQoS(p) => encode_u8_property(self.into(), *p as u8, dest),

            Property::RetainAvail(p) |
            Property::WildcardSubAvail(p) |
            Property::SubIdAvail(p) |
            Property::ShardSubAvail(p) => encode_bool_property(self.into(), *p, dest),

            Property::CorrelationData(p) |
            Property::AuthData(p) => encode_bin_property(self.into(), p, dest)?,

            Property::UserProperty(k, v) => {
                dest.put_u8(PropertyType::from(self) as u8);
                put_utf8(k, dest)?;
                put_utf8(v, dest)?;
            }
                
        }
        Ok(())
    }
}

pub trait PropertyEncode {
    fn property_encode() -> Result<(), MqttCodecError>;
}

pub trait PropertySize {
    fn property_size_internal() -> u32;
}

impl From<&Property> for PropertyType {
    fn from(value: &Property) -> Self {
        match value {
            Property::PayloadFormat(_) => PropertyType::PayloadFormat,
            Property::MessageExpiry(_) => PropertyType::MessageExpiry,
            Property::ContentType(_) => PropertyType::ContentType,
            Property::ResponseTopic(_) => PropertyType::ResponseTopic,
            Property::CorrelationData(_) => PropertyType::CorrelationData,
            Property::SubscriptionId(_) => PropertyType::SubscriptionId,
            Property::SessionExpiryInt(_) => PropertyType::SessionExpiryInt,
            Property::AssignedClientId(_) => PropertyType::AssignedClientId,
            Property::KeepAlive(_) => PropertyType::KeepAlive,
            Property::AuthMethod(_) => PropertyType::AuthMethod,
            Property::AuthData(_) => PropertyType::AuthData,
            Property::ReqProblemInfo(_) => PropertyType::ReqProblemInfo,
            Property::WillDelay(_) => PropertyType::WillDelay,
            Property::ReqRespInfo(_) => PropertyType::ReqRespInfo,
            Property::RespInfo(_) => PropertyType::RespInfo,
            Property::ServerRef(_) => PropertyType::ServerRef,
            Property::Reason(_) => PropertyType::Reason,
            Property::RecvMax(_) => PropertyType::RecvMax,
            Property::TopicAliasMax(_) => PropertyType::TopicAliasMax,
            Property::TopicAlias(_) => PropertyType::TopicAlias,
            Property::MaxQoS(_) => PropertyType::MaxQoS,
            Property::RetainAvail(_) => PropertyType::RetainAvail,
            Property::UserProperty(_, _) => PropertyType::UserProperty,
            Property::MaxPacketSize(_) => PropertyType::MaxPacketSize,
            Property::WildcardSubAvail(_) => PropertyType::WildcardSubAvail,
            Property::SubIdAvail(_) => PropertyType::SubIdAvail,
            Property::ShardSubAvail(_) => PropertyType::ShardSubAvail,
        }
    }
}


