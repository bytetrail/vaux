mod codec;
mod connect;
pub use crate::codec::{FixedHeader, PacketType, MQTTCodec, MQTTCodecError};





#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum QoSLevel {
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}

struct QoSParseError {}

#[derive(Debug)]
pub struct WillMessage {
    qos: QoSLevel,
    retain: bool,
    topic: String,
    message: Vec<u8>,
}

impl TryFrom<u8> for QoSLevel {
    type Error = QoSParseError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(QoSLevel::AtMostOnce),
            0x01 => Ok(QoSLevel::AtLeastOnce),
            0x02 => Ok(QoSLevel::ExactlyOnce),
            _ => Err(QoSParseError{})
        }
    }
}

impl WillMessage {
    fn new(qos: QoSLevel, retain: bool) -> Self {
        WillMessage {
            qos,
            retain,
            topic: "".to_string(),
            message: Vec::new()
        }
    }
}

#[derive(Debug)]
struct AuthData {
    method: String,
    data: Vec<u8>,
}

#[cfg(test)]
mod test {
    use super::*;
}
