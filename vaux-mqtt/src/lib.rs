pub mod codec;

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
pub use crate::codec::{ControlPacket, PacketType, MQTTCodec, MQTTCodecError};


/// MQTT property type. For more information on the specific property types,
/// please see the
/// [MQTT Specification](https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901027).
/// Identifier | Name | Type
/// -----------+------+-----
/// 0x01 | Payload Format Indicator | byte
/// 0x02 | Message Expiry Interval | 4 byte Integer
/// 0x03 | Content Type | UTF-8 string
/// 0x08 | Response Topic | UTF-8 string
/// 0x09 | Correlation Data | Binary Data
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
/// 0x23 | Topic Alias | 2 byte integer
/// 0x24 | Maximum QoS | byte
/// 0x25 | Retain Available | byte
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PropertyType {
    PayloadFormat = 0x01,
    MessageExpiry = 0x02,
    ContentType = 0x03,
    ResponseTopic = 0x08,
    CorrelationData = 0x09,
    SubscriptionId = 0x0b,
    SessionExpiry = 0x11,
    ClientId = 0x12,
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
}


#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum QoSLevel {
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}


pub struct WillMessage {
    qos: QoSLevel,
    retain: bool,
    topic: String,
    message: Vec<u8>,
}

struct AuthData {
    method: String,
    data: Vec<u8>,
}

struct UserProperty {
    key: String,
    value: String,
}

pub struct Connect {
    clean_start: bool,
    keep_alive: u16,
    session_expiry_interval: u32,
    receive_max: u16,
    max_packet_size: u32,
    topic_alias_max: u16,
    req_resp_info: bool,
    request_info: bool,
    auth: Option<AuthData>,
    will_message: Option<WillMessage>,
    user_property: Option<HashMap<String, String>>,
}

#[cfg(test)]
mod test {
    use super::*;


}
