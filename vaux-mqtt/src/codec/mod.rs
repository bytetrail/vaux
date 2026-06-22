pub mod fixed;
pub use fixed::FixedHeader;
use crate::{
    ConnAck, Disconnect, Subscribe, connect::Connect, publish::Publish, pubresp::{PubAck, PubComp, PubRec, PubRel}, subscribe::SubAck, unsubscribe::{UnsubAck, Unsubscribe}
};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::fmt::{Display, Formatter};

pub(crate) const PACKET_RESERVED_NONE: u8 = 0x00;
pub(crate) const PACKET_RESERVED_BIT1: u8 = 0x02;

pub const MIN_VARIABLE_BYTE_INT: u32 = 0;
pub const MAX_VARIABLE_BYTE_INT: u32 = 268_435_455; // 256 MB

pub trait PropertyCodecSize {
    fn property_size(&self) -> u32;
}

pub trait CodecSize {
    fn codec_size(&self) -> u32;
}

pub trait PropertyEncode {
    fn property_encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError>;
}

pub trait PropertyDecode {
    fn property_decode(&mut self, src: &mut BytesMut) -> Result<usize, MqttCodecError>;
}   

pub trait Encode {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError>;
}

pub trait Decode {
    fn decode(&mut self, src: &mut BytesMut) -> Result<usize, MqttCodecError>;
}



/// MQTT Control Packet Type
/// #[repr(u8)]
#[derive(Default, Debug, PartialEq, Eq, Copy, Clone, Hash)]
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
        write!(f, "{self:?}")
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
            value => Err(MqttCodecError::new(&format!(
                " Unsupported reason code: {value}"
            ))),
        }
    }
}

impl CodecSize for Reason {
    fn codec_size(&self) -> u32 {
        1
    }
}

impl Encode for Reason {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        dest.put_u8(*self as u8);
        Ok(())
    }
}

impl Decode for Reason {
    fn decode(&mut self, src: &mut BytesMut) -> Result<usize, MqttCodecError> {
        let val = src.get_u8();
        *self = val.try_into()?;
        Ok(1)
    }
}

pub fn codec_size_vec_reason(reason_codes: &Vec<Reason>) -> u32 {
    reason_codes.len() as u32
}

impl Encode for Vec<Reason> {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        self.iter().for_each(|reason| dest.put_u8(*reason as u8));
        Ok(())
    }
}

impl Decode for Vec<Reason> {
    fn decode(&mut self, src: &mut BytesMut) -> Result<usize, MqttCodecError> {
        let len = src.remaining();
        for _ in 0..len {
            let reason = src.get_u8().try_into()?;
            self.push(reason);
        }
        Ok(len)
    }
}


#[allow(clippy::enum_variant_names)]
#[repr(u8)]
#[derive(Default, Debug, PartialEq, Eq, Copy, Clone, Hash)]
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
                "{value} is not a value QoSLevel"
            ))),
        }
    }
}

impl Encode for QoSLevel {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        dest.put_u8(*self as u8);
        Ok(())
    }
}

impl Decode for QoSLevel {
    fn decode(&mut self, src: &mut BytesMut) -> Result<usize, MqttCodecError> {
        let val = src.get_u8();
        *self = val.try_into()?;
        Ok(1)
    }
}

impl CodecSize for QoSLevel {
    fn codec_size(&self) -> u32 {
        1
    }
}

impl PropertyCodecSize for &QoSLevel {
    fn property_size(&self) -> u32 {
        2
    }
}

impl CodecSize for &String {
    fn codec_size(&self) -> u32 {
        2 + self.len() as u32
    }
}

impl Encode for &String {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        encode_string(self, dest)
    }
}



#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Packet {
    PingRequest(crate::PingReq),
    PingResponse(crate::PingResp),
    Connect(Box<Connect>),
    ConnAck(ConnAck),
    Publish(Publish),
    PubAck(PubAck),
    PubComp(PubComp),
    PubRec(PubRec),
    PubRel(PubRel),
    Disconnect(Disconnect),
    Subscribe(Subscribe),
    SubAck(SubAck),
    Unsubscribe(Unsubscribe),
    UnsubAck(UnsubAck),
}

impl Encode for Packet {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        match self {
            Packet::PingRequest(pingreq) => pingreq.encode(dest),
            Packet::PingResponse(pingresp) => pingresp.encode(dest),
            Packet::Connect(connect) => connect.encode(dest),
            Packet::ConnAck(connack) => connack.encode(dest),
            Packet::Publish(publish) => publish.encode(dest),
            Packet::PubAck(puback) => puback.encode(dest),
            Packet::PubComp(pubcomp) => pubcomp.encode(dest),
            Packet::PubRec(pubrec) => pubrec.encode(dest),
            Packet::PubRel(pubrel) => pubrel.encode(dest),
            Packet::Disconnect(disconnect) => disconnect.encode(dest),
            Packet::Subscribe(subscribe) => subscribe.encode(dest),
            Packet::SubAck(suback) => suback.encode(dest),
            Packet::Unsubscribe(unsubscribe) => unsubscribe.encode(dest),
            Packet::UnsubAck(unsuback) => unsuback.encode(dest),
        }
    }
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
            Packet::Unsubscribe(_) => PacketType::Unsubscribe,
            Packet::UnsubAck(_) => PacketType::UnsubAck,
        }
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub enum ErrorKind {
    InsufficientData(usize, usize),
    #[default]
    MalformedPacket,
    InvalidPacket,
    UnsupportedQosLevel,
    UnsupportedResponseType,
    UnsupportedReason(u8),
    UnsupportedProperty(u8),
    InvalidUTF8,
    InvalidPacketIdentifier,
}

#[derive(Default, Debug)]
pub struct MqttCodecError {
    pub reason: String,
    pub kind: ErrorKind,
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
            ..Default::default()
        }
    }
}

impl std::error::Error for MqttCodecError {}

impl MqttCodecError {
    pub fn new(reason: &str) -> Self {
        MqttCodecError {
            reason: reason.to_string(),
            ..Default::default()
        }
    }

    pub fn new_with_kind(reason: &str, kind: ErrorKind) -> Self {
        MqttCodecError {
            reason: reason.to_string(),
            kind,
        }
    }
}

pub fn decode(src: &mut BytesMut) -> Result<Option<(Packet, usize)>, MqttCodecError> {
    let mut fixed_header = FixedHeader::default();
    let mut decode_len = fixed_header.decode(src)?;

    for idx in 0..=3 {
        if src[idx] & 0x80 != 0x00 {
            // insufficient bytes left to read remaining
            if src.remaining() < 1 {
                return Err(MqttCodecError::new_with_kind(
                    "Insufficient data",
                    ErrorKind::InsufficientData(1, src.remaining()),
                ));
            }
        } else {
            break;
        }
    }
    let (packet_remaining, bytes_read) = decode_variable_byte_int(src)?;
    if src.remaining() < packet_remaining as usize {
        return Err(MqttCodecError::new_with_kind(
            "Insufficient data",
            ErrorKind::InsufficientData(packet_remaining as usize, src.remaining()),
        ));
    }
    decode_len += bytes_read;
    match src.remaining() {
        val if val < packet_remaining as usize => {
            return Err(MqttCodecError {
                reason: format!(
                    "malformed packet: remaining length actual: {val} expected: {packet_remaining}",
                ),
                kind: ErrorKind::InsufficientData(packet_remaining as usize, val),
            })
        }
        val if val > packet_remaining as usize => {
            let total = src.remaining();
            let index = total - (total - packet_remaining as usize);
            _ = src.split_off(index);
        }
        _ => {}
    }
    match fixed_header.packet_type {
        PacketType::PingReq => Ok(Some((Packet::PingRequest(crate::PingReq::default()), decode_len))),
        PacketType::PingResp => Ok(Some((
            Packet::PingResponse(crate::PingResp::default()),
            decode_len,
        ))),
        PacketType::Connect => {
            let mut connect = Connect::new();
            decode_len += connect.decode(src)?;
            Ok(Some((Packet::Connect(Box::new(connect)), decode_len)))
        }
        PacketType::Publish => {
            let mut publish = Publish::new_from_header(fixed_header)?;
            decode_len += publish.decode(src)?;
            Ok(Some((Packet::Publish(publish), decode_len)))
        }
        PacketType::PubAck => {
            let mut puback = PubAck::default();
            decode_len += puback.decode(src)?;
            Ok(Some((Packet::PubAck(puback), decode_len)))
        }
        PacketType::PubComp => {
            let mut pubcomp = PubComp::default();
            decode_len += pubcomp.decode(src)?;
            Ok(Some((Packet::PubComp(pubcomp), decode_len)))
        }
        PacketType::PubRec => {
            let mut pubrec = PubRec::default();
            decode_len += pubrec.decode(src)?;
            Ok(Some((Packet::PubRec(pubrec), decode_len)))
        }
        PacketType::PubRel => {
            let mut pubrel = PubRel::default();
            decode_len += pubrel.decode(src)?;
            Ok(Some((Packet::PubRel(pubrel), decode_len)))
        }
        PacketType::Disconnect => {
            let mut disconnect = Disconnect::default();
            decode_len += disconnect.decode(src)?;
            Ok(Some((Packet::Disconnect(disconnect), decode_len)))
        }
        PacketType::ConnAck => {
            let mut connack = ConnAck::default();
            decode_len += connack.decode(src)?;
            Ok(Some((Packet::ConnAck(connack), decode_len)))
        }
        PacketType::Subscribe => {
            let mut subscribe = Subscribe::default();
            decode_len += subscribe.decode(src)?;
            Ok(Some((Packet::Subscribe(subscribe), decode_len)))
        }
        PacketType::SubAck => {
            let mut suback = SubAck::default();
            decode_len += suback.decode(src)?;
            Ok(Some((Packet::SubAck(suback), decode_len)))
        }
        PacketType::Unsubscribe => {
            let mut unsubscribe = Unsubscribe::default();
            decode_len += unsubscribe.decode(src)?;
            Ok(Some((Packet::Unsubscribe(unsubscribe), decode_len)))
        }
        PacketType::UnsubAck => {
            let mut unsuback = UnsubAck::default();
            decode_len += unsuback.decode(src)?;
            Ok(Some((Packet::UnsubAck(unsuback), decode_len)))
        }
        _ => Err(MqttCodecError::new("unsupported packet type")),
    }
}

impl Decode for String {
    fn decode(&mut self, src: &mut BytesMut) -> Result<usize, MqttCodecError> {
        if src.len() < 2 {
            return Err(MqttCodecError::new("malformed Mqtt packet: string length"));
        }
        let len = src.get_u16();
        let mut dest_vec = vec![0; len as usize];

        src.try_copy_to_slice(&mut dest_vec).map_err(|e| {
            MqttCodecError::new_with_kind(
                format!("{e:?}").as_str(),
                ErrorKind::InsufficientData(len as usize, src.remaining()),
            )
        })?;
        *self = String::from_utf8(dest_vec).map_err(|e| MqttCodecError::new(&format!("{e:?}")))?;
        Ok(len as usize + 2)
    }
}

impl Encode for Vec<String> {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        for s in self {
            encode_string(s, dest)?;
        }
        Ok(())
    }
}

impl Decode for Vec<String> {
    fn decode(&mut self, src: &mut BytesMut) -> Result<usize, MqttCodecError> {
        let mut bytes_read = 0;
        while src.remaining() > 0 {
            let mut string = String::new();
            let var_bytes_read = string.decode(src)?;
            bytes_read += var_bytes_read;
            self.push(string);
        }
        Ok(bytes_read)
    }
}

impl Encode for Vec<u8> {
    fn encode(&self, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
        encode_array_field(self, dest)
    }
}

impl Decode for Vec<u8> {
    fn decode(&mut self, src: &mut BytesMut) -> Result<usize, MqttCodecError> {
        if src.remaining() < 2 {
            return Err(MqttCodecError::new("Insufficient data for binary length"));
        }
        let len = src.get_u16() as usize;
        self.resize(len, 0);
        let dest_buf: &mut [u8] = &mut self[0..len];
        src.try_copy_to_slice(dest_buf).map_err(|e| {
            MqttCodecError::new_with_kind(
                format!("{e:?}").as_str(),
                ErrorKind::InsufficientData(len, src.remaining()),
            )
        })?;

        Ok(len + 2)
    }
}

impl Decode for bool {
    fn decode(&mut self, src: &mut BytesMut) -> Result<usize, MqttCodecError> {
        *self = match src.get_u8() {
            0 => Ok(false),
            1 => Ok(true),
            v => Err(MqttCodecError::new(&format!(
                "invalid value {} for boolean property",
                v
            ))),
        }?;
        Ok(1)
    }
}

impl Decode for u8 {
    fn decode(&mut self, src: &mut BytesMut) -> Result<usize, MqttCodecError> {
        *self = src.get_u8();
        Ok(1)
    }
}

impl Decode for u16 {
    fn decode(&mut self, src: &mut BytesMut) -> Result<usize, MqttCodecError> {
        *self = src.get_u16();
        Ok(2)
    }
}

impl Decode for u32 {
    fn decode(&mut self, src: &mut BytesMut) -> Result<usize, MqttCodecError> {
        *self = src.get_u32();
        Ok(4)
    }
}

pub fn encode_string(src: &str, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
    let len = src.len();
    if len > u16::MAX as usize {
        return Err(MqttCodecError::new("string exceeds max length"));
    }
    dest.put_u16(len as u16);
    dest.put(src.as_bytes());
    Ok(())
}

pub fn decode_string(src: &mut BytesMut) -> Result<(String, usize), MqttCodecError> {
    let mut string = String::new();
    let bytes_read = string.decode(src)?;
    Ok((string, bytes_read))
}

pub fn encode_array_field(src: &[u8], dest: &mut BytesMut) -> Result<(), MqttCodecError> {
    let len = src.len();
    if len > u16::MAX as usize {
        return Err(MqttCodecError::new("binary data exceeds max length"));
    }
    dest.put_u16(len as u16);
    dest.put(src);
    Ok(())
}

pub fn decode_array_field(src: &mut BytesMut) -> Result<(Vec<u8>, usize), MqttCodecError> {
    let mut data = Vec::new();
    let bytes_read = data.decode(src)?;
    Ok((data, bytes_read))
}

/// Returns the length of an encoded MQTT variable length unsigned int
pub fn variable_byte_int_size(value: u32) -> u32 {
    match value {
        0..=127 => 1,
        128..=16383 => 2,
        16384..=2097151 => 3,
        _ => 4,
    }
}

pub fn variable_byte_int_size_ref(value: &u32) -> u32 {
    variable_byte_int_size(*value)
}

pub fn codec_size_opt_variable_byte_int_ref(src: &Option<u32>) -> u32 {
    match src {
        Some(val) => variable_byte_int_size(*val),
        None => 0,
    }
}

pub fn decode_opt_variable_byte_int(
    src: &mut BytesMut,
) -> Result<(Option<u32>, usize), MqttCodecError> {
    let (val, bytes_read) = decode_variable_byte_int(src)?;
    Ok((Some(val), bytes_read))
}

pub fn decode_variable_byte_int(src: &mut BytesMut) -> Result<(u32, usize), MqttCodecError> {
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
        if shift > 21 {
            return Err(MqttCodecError::new(
                "malformed packet: variable byte integer",
            ));
        }
    }
    Ok((result, shift / 7))
}

pub fn encode_variable_byte_int(val: u32, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
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
    Ok(())
}

pub fn encode_opt_variable_byte_int(val: Option<u32>, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
    if let Some(v) = val {
        encode_variable_byte_int(v, dest)
    } else {
        Ok(())
    }
}

pub fn encode_opt_variable_byte_int_ref(
    val: &Option<u32>,
    dest: &mut BytesMut,
) -> Result<(), MqttCodecError> {
    if let Some(v) = val {
        encode_variable_byte_int(*v, dest)
    } else {
        Ok(())
    }
}

pub fn encode_variable_byte_int_ref(val: &u32, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
    encode_variable_byte_int(*val, dest)
}

pub fn codec_size_vec_u8_raw(src: &Vec<u8>) -> u32 {
    src.len() as u32
}

pub fn encode_vec_u8_raw(src: &Vec<u8>, dest: &mut BytesMut) -> Result<(), MqttCodecError> {
    dest.put_slice(src);
    Ok(())
}

pub fn decode_vec_u8_raw(src: &mut BytesMut) -> Result<(Option<Vec<u8>>, usize), MqttCodecError> {
    let len = src.remaining();
    let mut dest = Vec::with_capacity(len);
    dest.resize(len, 0);
    let dest_buf: &mut [u8] = &mut dest[0..len];
    src.try_copy_to_slice(dest_buf).map_err(|e| {
        MqttCodecError::new_with_kind(
            format!("{e:?}").as_str(),
            ErrorKind::InsufficientData(len, src.remaining()),
        )
    })?;
    Ok((Some(dest), len))
}

pub fn decode_opt_vec_u8_raw(
    src: &mut BytesMut,
) -> Result<(Option<Bytes>, usize), MqttCodecError> {
    let len = src.remaining();
    let bytes = src.split_to(len).freeze();
    Ok((Some(bytes), len))
}


pub fn codec_size_opt_vec_u8_raw(src: &Option<Bytes>) -> u32 {
    match src {
        Some(bytes) => bytes.len() as u32,
        None => 0,
    }
}

pub fn encode_opt_vec_u8_raw_ref(
    src: &Option<Bytes>,
    dest: &mut BytesMut,
) -> Result<(), MqttCodecError> {
    if let Some(bytes) = src {
        dest.put_slice(bytes);
    }
    Ok(())
}

