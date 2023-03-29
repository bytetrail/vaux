use crate::publish::Publish;
use crate::subscribe::SubAck;
use crate::{ConnAck, Connect, Decode, Disconnect, Encode, FixedHeader, PropertyType, PubResp, Subscribe};
use bytes::{Buf, BufMut, BytesMut};
use std::collections::HashSet;
use std::fmt::{Display, Formatter};

pub(crate) const PROP_SIZE_U32: u32 = 5;
pub(crate) const PROP_SIZE_U16: u32 = 3;
pub(crate) const PROP_SIZE_U8: u32 = 2;
pub(crate) const PROP_SIZE_UTF8_STRING: u32 = 3;
pub(crate) const PROP_SIZE_BINARY: u32 = 3;
pub(crate) const SIZE_UTF8_STRING: u32 = 2;
pub(crate) const PACKET_RESERVED_NONE: u8 = 0x00;
pub(crate) const PACKET_RESERVED_BIT1: u8 = 0x02;

/// MQTT Control Packet Type
/// #[repr(u8)]
#[derive(Default, Debug, PartialEq, Eq, Copy, Clone)]
pub enum PacketType {
    #[default]
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
#[derive(Default, Debug, Eq, PartialEq, Copy, Clone)]
pub enum Reason {
    #[default]
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
    NotAuthorized,
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

impl Display for Reason {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[allow(non_upper_case_globals)]
impl Reason {
    pub const NormalDisconnect: Reason = Reason::Success;
    pub const GrantedQoS0: Reason = Reason::Success;
}

impl TryFrom<u8> for Reason {
    type Error = MqttCodecError;

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
            0x87 => Ok(Reason::NotAuthorized),
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
            value => Err(MqttCodecError::new(&format!("Invalid reason: {}", value))),
        }
    }
}

#[allow(clippy::enum_variant_names)]
#[repr(u8)]
#[derive(Default, Debug, PartialEq, Eq, Copy, Clone)]
pub enum QoSLevel {
    #[default]
    AtMostOnce = 0,
    AtLeastOnce = 1,
    ExactlyOnce = 2,
}

impl TryFrom<u8> for QoSLevel {
    type Error = MqttCodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(QoSLevel::AtMostOnce),
            0x01 => Ok(QoSLevel::AtLeastOnce),
            0x02 => Ok(QoSLevel::ExactlyOnce),
            value => Err(MqttCodecError::new(&format!(
                "{} is not a value QoSLevel",
                value
            ))),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Packet {
    PingRequest(FixedHeader),
    PingResponse(FixedHeader),
    Connect(Connect),
    ConnAck(ConnAck),
    Publish(Publish),
    PubAck(PubResp),
    PubComp(PubResp),
    PubRec(PubResp),
    PubRel(PubResp),
    Disconnect(Disconnect),
    Subscribe(Subscribe),
    SubAck(SubAck),
}

impl From<&Packet> for PacketType {
    fn from(p: &Packet) -> Self {
        match p {
            Packet::PingRequest(_) => PacketType::PingReq,
            Packet::PingResponse(_) => PacketType::PingResp,
            Packet::Connect(_) => PacketType::Connect,
            Packet::ConnAck(_) => PacketType::ConnAck,
            Packet::Publish(_) => PacketType::Publish,
            Packet::PubAck(_) => PacketType::PubAck,
            Packet::PubComp(_) => PacketType::PubComp,
            Packet::PubRec(_) => PacketType::PubRec,
            Packet::PubRel(_) => PacketType::PubRel,
            Packet::Disconnect(_) => PacketType::Disconnect,
            Packet::Subscribe(_) => PacketType::Subscribe,
            Packet::SubAck(_) => PacketType::SubAck,
        }
    }
}

#[derive(Debug)]
pub struct MqttCodecError {
    pub reason: String,
}

impl Display for MqttCodecError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mqtt codec error: {}", self.reason)
    }
}

impl From<std::io::Error> for MqttCodecError {
    fn from(_err: std::io::Error) -> Self {
        MqttCodecError {
            reason: "IO error".to_string(),
        }
    }
}

impl std::error::Error for MqttCodecError {}

impl MqttCodecError {
    pub fn new(reason: &str) -> Self {
        MqttCodecError {
            reason: reason.to_string(),
        }
    }
}

pub fn decode(src: &mut BytesMut) -> Result<Option<Packet>, MqttCodecError> {
    match decode_fixed_header(src) {
        Ok(packet_header) => match packet_header {
            Some(packet_header) => match packet_header.packet_type() {
                PacketType::PingReq => Ok(Some(Packet::PingRequest(packet_header))),
                PacketType::PingResp => Ok(Some(Packet::PingResponse(packet_header))),
                PacketType::Connect => {
                    let mut connect = Connect::default();
                    connect.decode(src)?;
                    Ok(Some(Packet::Connect(connect)))
                }
                PacketType::Publish => {
                    let mut publish = Publish::new_from_header(packet_header)?;
                    publish.decode(src)?;
                    Ok(Some(Packet::Publish(publish)))
                }
                PacketType::PubAck => {
                    let mut puback = PubResp::new_puback();
                    puback.decode(src)?;
                    Ok(Some(Packet::PubAck(puback)))
                }
                PacketType::PubComp => {
                    let mut pubcomp = PubResp::new_pubcomp();
                    pubcomp.decode(src)?;
                    Ok(Some(Packet::PubComp(pubcomp)))
                }
                PacketType::PubRec => {
                    let mut pubrec = PubResp::new_pubrec();
                    pubrec.decode(src)?;
                    Ok(Some(Packet::PubRec(pubrec)))
                }
                PacketType::PubRel => {
                    let mut pubrel = PubResp::new_pubrel();
                    pubrel.decode(src)?;
                    Ok(Some(Packet::PubRel(pubrel)))
                }
                PacketType::Disconnect => {
                    let mut disconnect = Disconnect::default();
                    disconnect.decode(src)?;
                    Ok(Some(Packet::Disconnect(disconnect)))
                }
                PacketType::ConnAck => {
                    let mut connack = ConnAck::default();
                    connack.decode(src)?;
                    Ok(Some(Packet::ConnAck(connack)))
                }
                PacketType::Subscribe => {
                    let mut subscribe = Subscribe::default();
                    subscribe.decode(src)?;
                    Ok(Some(Packet::Subscribe(subscribe)))
                }
                PacketType::SubAck => {
                    let mut suback = SubAck::default();
                    suback.decode(src)?;
                    Ok(Some(Packet::SubAck(suback)))
                }
                _ => Err(MqttCodecError::new("unsupported packet type")),
            },
            None => Ok(None),
        },
        Err(e) => Err(e),
    }
}

pub fn encode(packet: Packet, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
    match packet {
        Packet::Connect(c) => c.encode(dest),
        Packet::ConnAck(c) => c.encode(dest),
        Packet::Disconnect(d) => d.encode(dest),
        Packet::Publish(p) => p.encode(dest),
        Packet::PubAck(p) |
        Packet::PubComp(p) |
        Packet::PubRec(p) |
        Packet::PubRel(p) => p.encode(dest),

        Packet::PingRequest(header) | Packet::PingResponse(header) => {
            dest.put_u8(header.packet_type() as u8 | header.flags());
            dest.put_u8(0x_00);
            Ok(())
        }
        Packet::Subscribe(s) => s.encode(dest),
        Packet::SubAck(s) => s.encode(dest),
    }?;
    Ok(())
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
) -> Result<(), MqttCodecError> {
    if properties.contains(&property) {
        return Err(MqttCodecError::new(
            format!("{} already set", property).as_str(),
        ));
    }
    properties.insert(property);
    Ok(())
}

pub(crate) fn encode_u8_property(property_type: PropertyType, value: u8, dest: &mut BytesMut) {
    dest.put_u8(property_type as u8);
    dest.put_u8(value);
}

pub(crate) fn encode_bool_property(property_type: PropertyType, value: bool, dest: &mut BytesMut) {
    dest.put_u8(property_type as u8);
    dest.put_u8(value as u8);
}

pub(crate) fn encode_u16_property(property_type: PropertyType, value: u16, dest: &mut BytesMut) {
    dest.put_u8(property_type as u8);
    dest.put_u16(value);
}

pub(crate) fn encode_u32_property(property_type: PropertyType, value: u32, dest: &mut BytesMut) {
    dest.put_u8(property_type as u8);
    dest.put_u32(value);
}

pub(crate) fn encode_utf8_property(
    property_type: PropertyType,
    value: &str,
    dest: &mut BytesMut,
) -> Result<(), MqttCodecError> {
    dest.put_u8(property_type as u8);
    put_utf8(value, dest)
}

pub(crate) fn encode_bin_property(
    property_type: PropertyType,
    value: &[u8],
    dest: &mut BytesMut,
) -> Result<(), MqttCodecError> {
    dest.put_u8(property_type as u8);
    put_bin(value, dest)
}

pub(crate) fn encode_var_int_property(
    property_type: PropertyType,
    value: u32,
    dest: &mut BytesMut,
) {
    dest.put_u8(property_type as u8);
    put_var_u32(value, dest);
}

pub(crate) fn decode_prop_bool(src: &mut BytesMut) -> Result<bool, MqttCodecError> {
    match src.get_u8() {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(MqttCodecError::new(&format!(
            "invalid property value: {}",
            value
        ))),
    }
}

#[cfg(not(feature = "pedantic"))]
pub fn get_bool(src: &mut BytesMut) -> Result<bool, MqttCodecError> {
    Ok(src.get_u8() != 0)
}

#[cfg(feature = "pedantic")]
pub fn get_bool(src: &mut BytesMut) -> Result<bool, MqttCodecError> {
    match src.get_u8() {
        0 => Ok(false),
        1 => Ok(true),
        v => Err(MqttCodecError::new(&format!(
            "invalid value {} for boolean property",
            v
        ))),
    }
}

pub(crate) fn put_utf8(src: &str, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
    let len = src.len();
    if len > u16::MAX as usize {
        return Err(MqttCodecError::new("string exceeds max length"));
    }
    dest.put_u16(src.len() as u16);
    dest.put(src.as_bytes());
    Ok(())
}

pub(crate) fn get_utf8(src: &mut BytesMut) -> Result<String, MqttCodecError> {
    let len = src.get_u16();
    if src.remaining() < len as usize {
        return Err(MqttCodecError::new("malformed Mqtt packet: string length"));
    }
    let mut chars: Vec<u8> = Vec::with_capacity(len as usize);
    for _ in 0..len {
        chars.push(src.get_u8());
    }
    match String::from_utf8(chars) {
        Ok(s) => Ok(s),
        Err(e) => Err(MqttCodecError::new(&format!("{:?}", e))),
    }
}

pub(crate) fn get_bin(src: &mut BytesMut) -> Result<Vec<u8>, MqttCodecError> {
    let mut dest = Vec::new();
    let len = src.get_u16() as usize;
    dest.resize(len, 0);
    for _ in 0..len {
        dest.push(src.get_u8());
    }
    Ok(dest)
}

pub(crate) fn put_bin(src: &[u8], dest: &mut BytesMut) -> Result<(), MqttCodecError> {
    let len = src.len();
    if len > u16::MAX as usize {
        return Err(MqttCodecError::new("binary data exceeds max length"));
    }
    dest.put_u16(len as u16);
    dest.put(src);
    Ok(())
}

pub(crate) fn put_var_u32(val: u32, dest: &mut BytesMut) {
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

pub(crate) fn get_var_u32(src: &mut BytesMut) -> u32 {
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

pub fn decode_fixed_header(src: &mut BytesMut) -> Result<Option<FixedHeader>, MqttCodecError> {
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
    let remaining = get_var_u32(src);
    if src.remaining() != remaining as usize {
        // TODO return a protocol error
        return Ok(None);
    }
    match packet_type {
        PacketType::Connect
        | PacketType::PubRel
        | PacketType::PubAck
        | PacketType::Subscribe
        | PacketType::Unsubscribe => {
            if flags != PACKET_RESERVED_NONE {
                MqttCodecError::new(
                    format!("invalid flags for {}: {}", packet_type, flags).as_str(),
                );
            }
            Ok(Some(FixedHeader::new_with_remaining(packet_type, remaining)))
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
                MqttCodecError::new(
                    format!("invalid flags for {}: {}", packet_type, flags).as_str(),
                );
            }
            Ok(Some(FixedHeader::new_with_remaining(packet_type, remaining)))
        }
        PacketType::Publish => {
            let mut header = FixedHeader::new_with_remaining(packet_type, remaining);
            header.set_flags(flags)?;
            Ok(Some(header))
        }
        _ => Err(MqttCodecError::new("unexpected packet type ")),
    }
}
