use crate::connect::Connect;
use crate::{Encode, FixedHeader, Packet, QoSParseError, PACKET_RESERVED_NONE};
use bytes::{Buf, BufMut, BytesMut};
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use tokio_util::codec::{Decoder, Encoder};

pub(crate) const PROP_SIZE_U32: u32 = 5;
pub(crate) const PROP_SIZE_U16: u32 = 3;
pub(crate) const PROP_SIZE_U8: u32 = 2;

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
    SessionExpiry = 0x11,
    AssignedClientId = 0x12,
    KeepAlive = 0x13,
    AuthMethod = 0x15,
    AuthData = 0x16,
    RequestInfo = 0x17,
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
            PropertyType::SessionExpiry => write!(f, "\"Session Expiry Interval\""),
            PropertyType::AssignedClientId => write!(f, "\"Assigned Client Identifier\""),
            PropertyType::KeepAlive => write!(f, "\"Server Keep Alive\""),
            PropertyType::AuthMethod => write!(f, "\"Authentication Method\""),
            PropertyType::AuthData => write!(f, "\"Authentication Data\""),
            PropertyType::RequestInfo => write!(f, "\"Request Problem Information\""),
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
            0x11 => Ok(PropertyType::SessionExpiry),
            0x12 => Ok(PropertyType::AssignedClientId),
            0x13 => Ok(PropertyType::KeepAlive),
            0x15 => Ok(PropertyType::AuthMethod),
            0x16 => Ok(PropertyType::AuthData),
            0x17 => Ok(PropertyType::RequestInfo),
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
            _ => Err(MQTTCodecError::new("unexpected property type value")),
        }
    }
}

/// MQTT Control Packet Type
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum PacketType {
    Connect = 0x10,
    ConnAck = 0x20,
    Publish = 0x30,
    PubAck = 0x40,
    PubRec = 0x50,
    PubRel = 0x60,
    PubComp = 0x70,
    Subscribe = 0x80,
    SubAck = 0x90,
    Unsubscribe = 0xa0,
    UnsubAck = 0xb0,
    PingReq = 0xc0,
    PingResp = 0xd0,
    Disconnect = 0xe0,
    Auth = 0xf0,
}

impl From<u8> for PacketType {
    fn from(val: u8) -> Self {
        match val & 0xf0 {
            0x10 => PacketType::Connect,
            0x20 => PacketType::ConnAck,
            0x30 => PacketType::Publish,
            0x40 => PacketType::PubAck,
            0x50 => PacketType::PubRec,
            0x60 => PacketType::PubRel,
            0x70 => PacketType::PubComp,
            0x80 => PacketType::Subscribe,
            0x90 => PacketType::SubAck,
            0xa0 => PacketType::Unsubscribe,
            0xb0 => PacketType::UnsubAck,
            0xc0 => PacketType::PingReq,
            0xd0 => PacketType::PingResp,
            0xe0 => PacketType::Disconnect,
            _ => PacketType::Auth,
        }
    }
}

impl Display for PacketType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{:?}", &self).as_str().to_uppercase())
    }
}

/// Reason code is a 1 byte unsigned value that indicates the result of a control
/// packet request. For more information on reason codes see the MQTT Specification,
/// <https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901031>
#[repr(u8)]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum Reason {
    Success,
    GrantedQoS1,
    GrantedQoS2,
    DisconnectWillMsg = 0x04,
    NoSubscribers = 0x10,
    NoSubscriptionExisted,
    ContinueAuth = 0x18,
    Reauthenticate,
    UnspecifiedErr = 0x80,
    MalformedPacket,
    ProtocolErr,
    ImplementationErr,
    UnsupportedProtocolVersion,
    InvalidClientId,
    AuthenticationErr,
    Unauthorized,
    ServerUnavailable,
    ServerBusy,
    Banned,
    ServerShutdown,
    AuthMethodErr,
    KeepAliveTimeout,
    SessionTakeOver,
    InvalidTopicFilter,
    InvalidTopicName,
    PacketIdInUse,
    PacketIdNotFound,
    ReceiveMaxExceeded,
    InvalidTopicAlias,
    PacketTooLarge,
    MessageRate,
    QuotaExceeded,
    AdminAction,
    PayloadFormatErr,
    RetainUnsupported,
    QoSUnsupported,
    UseDiffServer,
    ServerMoved,
    SharedSubUnsupported,
    ConnRateExceeded,
    MaxConnectTime,
    SubIdUnsupported,
    WildcardSubUnsupported,
}

#[allow(non_upper_case_globals)]
impl Reason {
    pub const NormalDisconnect: Reason = Reason::Success;
    pub const GrantedQos0: Reason = Reason::Success;
}

impl TryFrom<u8> for Reason {
    type Error = MQTTCodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Reason::Success),
            0x01 => Ok(Reason::GrantedQoS1),
            0x02 => Ok(Reason::GrantedQoS2),
            0x04 => Ok(Reason::DisconnectWillMsg),
            0x10 => Ok(Reason::NoSubscribers),
            0x11 => Ok(Reason::NoSubscriptionExisted),
            0x18 => Ok(Reason::ContinueAuth),
            0x19 => Ok(Reason::Reauthenticate),
            0x80 => Ok(Reason::UnspecifiedErr),
            0x81 => Ok(Reason::MalformedPacket),
            0x82 => Ok(Reason::ProtocolErr),
            0x83 => Ok(Reason::ImplementationErr),
            0x84 => Ok(Reason::UnsupportedProtocolVersion),
            0x85 => Ok(Reason::InvalidClientId),
            0x86 => Ok(Reason::AuthenticationErr),
            0x87 => Ok(Reason::Unauthorized),
            0x88 => Ok(Reason::ServerUnavailable),
            0x89 => Ok(Reason::ServerBusy),
            0x8a => Ok(Reason::Banned),
            0x8b => Ok(Reason::ServerShutdown),
            0x8c => Ok(Reason::AuthMethodErr),
            0x8d => Ok(Reason::KeepAliveTimeout),
            0x8e => Ok(Reason::SessionTakeOver),
            0x8f => Ok(Reason::InvalidTopicFilter),
            0x90 => Ok(Reason::InvalidTopicName),
            0x91 => Ok(Reason::PacketIdInUse),
            0x92 => Ok(Reason::PacketIdNotFound),
            0x93 => Ok(Reason::ReceiveMaxExceeded),
            0x94 => Ok(Reason::InvalidTopicAlias),
            0x95 => Ok(Reason::PacketTooLarge),
            0x96 => Ok(Reason::MessageRate),
            0x97 => Ok(Reason::QuotaExceeded),
            0x98 => Ok(Reason::AdminAction),
            0x99 => Ok(Reason::PayloadFormatErr),
            0x9a => Ok(Reason::RetainUnsupported),
            0x9b => Ok(Reason::QoSUnsupported),
            0x9c => Ok(Reason::UseDiffServer),
            0x9d => Ok(Reason::ServerMoved),
            0x9e => Ok(Reason::SharedSubUnsupported),
            0x9f => Ok(Reason::ConnRateExceeded),
            0xa0 => Ok(Reason::MaxConnectTime),
            0xa1 => Ok(Reason::SubIdUnsupported),
            0xa2 => Ok(Reason::WildcardSubUnsupported),
            value => Err(MQTTCodecError::new(&format!("Invalid reason: {}", value))),
        }
    }
}

#[derive(Debug)]
pub struct MQTTCodecError {
    reason: String,
}

impl Display for MQTTCodecError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "MQTT codec error: {}", self.reason)
    }
}

impl From<QoSParseError> for MQTTCodecError {
    fn from(_: QoSParseError) -> Self {
        MQTTCodecError {
            reason: "invalid QOS level".to_string(),
        }
    }
}

impl From<std::io::Error> for MQTTCodecError {
    fn from(_err: std::io::Error) -> Self {
        MQTTCodecError {
            reason: "IO error".to_string(),
        }
    }
}

impl std::error::Error for MQTTCodecError {}

impl MQTTCodecError {
    pub fn new(reason: &str) -> Self {
        MQTTCodecError {
            reason: reason.to_string(),
        }
    }
}

/// Returns the length of an encoded MQTT variable length unsigned int
pub(crate) fn variable_byte_int_size(value: u32) -> u32 {
    match value {
        0..=127 => 1,
        128..=16383 => 2,
        16384..=2097151 => 3,
        _ => 4,
    }
}

pub(crate) fn check_property(
    property: PropertyType,
    properties: &mut HashSet<PropertyType>,
) -> Result<(), MQTTCodecError> {
    if properties.contains(&property) {
        return Err(MQTTCodecError::new(
            format!("{} already set", property).as_str(),
        ));
    }
    properties.insert(property);
    Ok(())
}

pub(crate) fn decode_prop_bool(src: &mut BytesMut) -> Result<bool, MQTTCodecError> {
    match src.get_u8() {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(MQTTCodecError::new(&format!(
            "invalid property value: {}",
            value
        ))),
    }
}

pub(crate) fn encode_utf8_string(src: &str, dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
    let len = src.len();
    if len > u16::MAX as usize {
        return Err(MQTTCodecError::new("string exceeds max length"));
    }
    dest.put_u16(src.len() as u16);
    dest.put(src.as_bytes());
    Ok(())
}

pub(crate) fn decode_utf8_string(src: &mut BytesMut) -> Result<String, MQTTCodecError> {
    let len = src.get_u16();
    if src.remaining() < len as usize {
        return Err(MQTTCodecError::new("malformed MQTT packet: string length"));
    }
    let mut chars: Vec<u8> = Vec::with_capacity(len as usize);
    for _ in 0..len {
        chars.push(src.get_u8());
    }
    match String::from_utf8(chars) {
        Ok(s) => Ok(s),
        Err(e) => Err(MQTTCodecError::new(&format!("{:?}", e))),
    }
}

pub(crate) fn decode_binary_data(src: &mut BytesMut) -> Result<Vec<u8>, MQTTCodecError> {
    let mut dest = Vec::new();
    let len = src.get_u16() as usize;
    dest.resize(len, 0);
    for _ in 0..len {
        dest.push(src.get_u8());
    }
    Ok(dest)
}

pub(crate) fn encode_binary_data(src: &[u8], dest: &mut BytesMut) -> Result<(), MQTTCodecError> {
    let len = src.len();
    if len > u16::MAX as usize {
        return Err(MQTTCodecError::new("binary data exceeds max length"));
    }
    dest.put_u16(len as u16);
    dest.put(src);
    Ok(())
}

pub(crate) fn encode_variable_len_integer(val: u32, dest: &mut BytesMut) {
    let mut encode = true;
    let mut input_val = val;
    while encode {
        let mut next_byte = (input_val % 0x80) as u8;
        input_val >>= 7;
        if input_val > 0 {
            next_byte |= 0x80;
        } else {
            encode = false;
        }
        dest.put_u8(next_byte);
    }
}

pub(crate) fn decode_variable_len_integer(src: &mut BytesMut) -> u32 {
    let mut result = 0_u32;
    let mut shift = 0;
    let mut next_byte = src.get_u8();
    let mut decode = true;
    while decode {
        result += ((next_byte & 0x7f) as u32) << shift;
        shift += 7;
        if next_byte & 0x80 == 0 {
            decode = false;
        } else {
            next_byte = src.get_u8();
        }
    }
    result
}

#[derive(Debug)]
pub struct MQTTCodec {}

impl Decoder for MQTTCodec {
    type Item = Packet;
    type Error = MQTTCodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match decode_fixed_header(src) {
            Ok(packet_header) => {
                match packet_header {
                    Some(packet_header) => {
                        match packet_header.packet_type {
                            PacketType::PingReq => Ok(Some(Packet::PingRequest(packet_header))),
                            PacketType::PingResp => Ok(Some(Packet::PingResponse(packet_header))),
                            PacketType::Connect => {
                                let mut connect = Connect::default();
                                connect.decode(src)?;
                                Ok(Some(Packet::Connect(connect)))
                            }
                            PacketType::Publish => Ok(None),
                            //PacketType::ConnAck => Ok(Some(Packet::ConnAck(packet_header))),
                            _ => Err(MQTTCodecError::new("unsupported packet type")),
                        }
                    }
                    None => Ok(None),
                }
            }
            Err(e) => {
                println!("Error: {:?}", e);
                Err(e)
            }
        }
    }
}

impl Encoder<Packet> for MQTTCodec {
    type Error = MQTTCodecError;

    fn encode(&mut self, packet: Packet, dest: &mut BytesMut) -> Result<(), Self::Error> {
        match packet {
            Packet::Connect(c) => c.encode(dest),
            Packet::ConnAck(c) => c.encode(dest),
            Packet::PingRequest(header) | Packet::PingResponse(header) => {
                dest.put_u8(header.packet_type as u8 | header.flags);
                dest.put_u8(0x_00);
                Ok(())
            }
            _ => return Err(MQTTCodecError::new("unsupported packet type")),
        }?;
        Ok(())
    }
}

fn decode_fixed_header(src: &mut BytesMut) -> Result<Option<FixedHeader>, MQTTCodecError> {
    if src.remaining() < 2 {
        return Ok(None);
    }
    for idx in 1..=3 {
        if src[idx] & 0x80 != 0x00 {
            // insufficient bytes left to read remaining
            if src.remaining() < 1 {
                return Ok(None);
            }
        } else {
            break;
        }
    }
    let first_byte = src.get_u8();
    let packet_type = PacketType::from(first_byte);
    let flags = first_byte & 0x0f;
    let remaining = decode_variable_len_integer(src);
    if src.remaining() != remaining as usize {
        return Ok(None);
    }
    match packet_type {
        PacketType::Connect
        | PacketType::PubRel
        | PacketType::Subscribe
        | PacketType::Unsubscribe => {
            if flags != PACKET_RESERVED_NONE {
                MQTTCodecError::new(
                    format!("invalid flags for {}: {}", packet_type, flags).as_str(),
                );
            }
            Ok(Some(FixedHeader {
                packet_type,
                flags,
                remaining,
            }))
        }
        PacketType::ConnAck
        | PacketType::PubRec
        | PacketType::PubComp
        | PacketType::SubAck
        | PacketType::UnsubAck
        | PacketType::PingReq
        | PacketType::PingResp
        | PacketType::Disconnect
        | PacketType::Auth => {
            if flags != PACKET_RESERVED_NONE {
                MQTTCodecError::new(
                    format!("invalid flags for {}: {}", packet_type, flags).as_str(),
                );
            }
            Ok(Some(FixedHeader {
                packet_type,
                flags,
                remaining,
            }))
        }
        PacketType::Publish => Ok(Some(FixedHeader {
            packet_type,
            flags,
            remaining,
        })),
        _ => Err(MQTTCodecError::new("unexpected packet type ")),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    /// Random test of display trait for packet types.
    fn test_packet_type_display() {
        let p = PacketType::UnsubAck;
        assert_eq!("UNSUBACK", format!("{}", p));
        let p = PacketType::PingReq;
        assert_eq!("PINGREQ", format!("{}", p));
    }

    #[test]
    fn test_control_packet_type_from() {
        let val = 0x12;
        assert_eq!(
            PacketType::Connect,
            PacketType::from(val),
            "expected {:?}",
            PacketType::Connect
        );
        let val = 0x2f;
        assert_eq!(
            PacketType::ConnAck,
            PacketType::from(val),
            "expected {:?}",
            PacketType::ConnAck
        );
        let val = 0x35;
        assert_eq!(
            PacketType::Publish,
            PacketType::from(val),
            "expected {:?}",
            PacketType::Publish
        );
        let val = 0x47;
        assert_eq!(
            PacketType::PubAck,
            PacketType::from(val),
            "expected {:?}",
            PacketType::PubAck
        );
        let val = 0x5f;
        assert_eq!(
            PacketType::PubRec,
            PacketType::from(val),
            "expected {:?}",
            PacketType::PubRec
        );
        let val = 0x6f;
        assert_eq!(
            PacketType::PubRel,
            PacketType::from(val),
            "expected {:?}",
            PacketType::PubRel
        );
        let val = 0x7f;
        assert_eq!(
            PacketType::PubComp,
            PacketType::from(val),
            "expected {:?}",
            PacketType::PubComp
        );
        let val = 0x8f;
        assert_eq!(
            PacketType::Subscribe,
            PacketType::from(val),
            "expected {:?}",
            PacketType::Subscribe
        );
        let val = 0x9f;
        assert_eq!(
            PacketType::SubAck,
            PacketType::from(val),
            "expected {:?}",
            PacketType::SubAck
        );
        let val = 0xaf;
        assert_eq!(
            PacketType::Unsubscribe,
            PacketType::from(val),
            "expected {:?}",
            PacketType::Unsubscribe
        );
        let val = 0xbf;
        assert_eq!(
            PacketType::UnsubAck,
            PacketType::from(val),
            "expected {:?}",
            PacketType::UnsubAck
        );
        let val = 0xcf;
        assert_eq!(
            PacketType::PingReq,
            PacketType::from(val),
            "expected {:?}",
            PacketType::PingReq
        );
        let val = 0xdf;
        assert_eq!(
            PacketType::PingResp,
            PacketType::from(val),
            "expected {:?}",
            PacketType::PingResp
        );
        let val = 0xff;
        assert_eq!(
            PacketType::Auth,
            PacketType::from(val),
            "expected {:?}",
            PacketType::Auth
        );
    }

    #[test]
    fn test_encode_var_int() {
        let test = 128_u32;
        let mut encoded: BytesMut = BytesMut::with_capacity(6);
        encode_variable_len_integer(test, &mut encoded);
        assert_eq!(0x80, encoded[0]);
        assert_eq!(0x01, encoded[1]);
        let test = 777;
        let mut encoded: BytesMut = BytesMut::with_capacity(6);
        encode_variable_len_integer(test, &mut encoded);
        assert_eq!(0x89, encoded[0]);
        assert_eq!(0x06, encoded[1]);
    }

    #[test]
    fn test_decode_var_int() {
        let mut encoded: BytesMut = BytesMut::with_capacity(6);
        // 0x80
        encoded.put_u8(0x80);
        encoded.put_u8(0x01);
        let val = decode_variable_len_integer(&mut encoded);
        assert_eq!(128, val);
        // 777 --- 0x309
        encoded.clear();
        encoded.put_u8(0x89);
        encoded.put_u8(0x06);
        let val = decode_variable_len_integer(&mut encoded);
        assert_eq!(777, val);
    }
}
