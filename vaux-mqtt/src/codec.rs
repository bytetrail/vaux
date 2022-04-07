use bytes::{BufMut, BytesMut};
use std::fmt::{Display, Formatter};
use tokio_util::codec::{Decoder, Encoder};



const PACKET_RESERVED_NONE: u8 = 0x00;
const PACKET_RESERVED_BIT1: u8 = 0x02;

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


#[derive(Debug)]
pub struct ControlPacket {
    packet_type: PacketType,
    flags: u8,
    remaining: u32,
}

impl ControlPacket {
    pub fn new(packet_type: PacketType) -> Self {
        match packet_type {
            PacketType::PubRel | PacketType::Subscribe | PacketType::Unsubscribe => ControlPacket {
                packet_type,
                flags: PACKET_RESERVED_BIT1,
                remaining: 0,
            },
            _ => ControlPacket {
                packet_type,
                flags: PACKET_RESERVED_NONE,
                remaining: 0,
            },
        }
    }

    pub fn packet_type(&self) -> PacketType {
        self.packet_type
    }

    pub fn set_remaining(&mut self, remaining: u32) {
        self.remaining = remaining;
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

#[derive(Debug)]
pub struct MQTTCodec {}

impl MQTTCodec {
    fn decode_remaining(&mut self, src: &mut BytesMut) -> (u32, usize) {
        let mut end = 5;
        // handle a degenerate case where there is a remaining length but bytes not present
        if src.len() < 5 {
            end = src.len();
        }
        decode_variable_len_integer(&src[1..end])
    }
}

impl Decoder for MQTTCodec {
    type Item = ControlPacket;
    type Error = MQTTCodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let packet_type = PacketType::from(src[0]);
        let flags = src[0] & 0x0f;
        match packet_type {
            PacketType::Connect
            | PacketType::PubRel
            | PacketType::Subscribe
            | PacketType::Unsubscribe => {
                let (remaining, _len) = self.decode_remaining(src);
                if flags != PACKET_RESERVED_NONE {
                    MQTTCodecError::new(format!("invalid flags for {}: {}", packet_type, flags).as_str());
                }
                Ok(Some(ControlPacket {
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
                let (remaining, _len) = self.decode_remaining(src);
                if flags != PACKET_RESERVED_NONE {
                    MQTTCodecError::new(format!("invalid flags for {}: {}", packet_type, flags).as_str());
                }
                Ok(Some(ControlPacket {
                    packet_type,
                    flags,
                    remaining,
                }))
            }
            _ => Err(MQTTCodecError::new("unexpected packet type")),
        }
    }
}

impl Encoder<ControlPacket> for MQTTCodec {
    type Error = MQTTCodecError;

    fn encode(&mut self, packet: ControlPacket, dest: &mut BytesMut) -> Result<(), Self::Error> {
        match packet.packet_type {
            PacketType::Connect
            | PacketType::ConnAck
            | PacketType::PingReq
            | PacketType::PingResp => {
                dest.put_u8(packet.packet_type as u8);
                dest.put_u8(packet.flags);
                Ok(())
            }
            _ => Err(MQTTCodecError::new(
                format!("unexpeccted packet type: {}", packet.packet_type).as_str(),
            )),
        }
    }
}

fn encode_variable_len_integer(val: u32, result: &mut [u8]) -> usize {
    let mut encode = true;
    let mut idx = 0;
    let mut input_val = val;
    while encode {
        let mut next_byte = (input_val % 0x80) as u8;
        input_val >>= 7;
        if input_val > 0 {
            next_byte |= 0x80;
        } else {
            encode = false;
        }
        result[idx] = next_byte;
        idx += 1;
    }
    idx
}

fn decode_variable_len_integer(data: &[u8]) -> (u32, usize) {
    let mut result = 0_u32;
    let mut shift = 0;
    let mut idx = 0_usize;
    let mut next_byte = data[0];
    let mut decode = true;
    while decode && idx < 4 {
        result += ((next_byte & 0x7f) as u32) << shift;
        shift += 7;
        idx += 1;
        if next_byte & 0x80 == 0 {
            decode = false;
        } else {
            next_byte = data[idx];
        }
    }
    (result, idx)
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
        let mut result = vec![0_u8; 4];
        let len = encode_variable_len_integer(test, &mut result[..]);
        assert_eq!(2, len);
        assert_eq!(0x80, result[0]);
        assert_eq!(0x01, result[1]);
        let test = 777;
        let mut result = vec![0_u8; 4];
        let len = encode_variable_len_integer(test, &mut result[..]);
        assert_eq!(2, len);
        assert_eq!(0x89, result[0]);
        assert_eq!(0x06, result[1]);
    }

    #[test]
    fn test_decode_var_int() {
        let mut test_value = [0_u8; 4];
        // 0x80
        test_value[0] = 0x80;
        test_value[1] = 0x01;
        let (val, len) = decode_variable_len_integer(&test_value);
        assert_eq!(2, len);
        assert_eq!(128, val);
        // 777 --- 0x309
        test_value[0] = 0x89;
        test_value[1] = 0x06;
        let (val, len) = decode_variable_len_integer(&test_value);
        assert_eq!(2, len);
        assert_eq!(777, val);
    }

    #[test]
    fn test_remaining() {
        let mut encoded: BytesMut = BytesMut::with_capacity(6);
        encoded.put_u8(PacketType::Connect as u8);
        let mut result = vec![0_u8; 4];
        let len = encode_variable_len_integer(12345, &mut result);
        encoded.put(&result[0..len]);
        let mut mqtt = MQTTCodec {};
        let result = mqtt.decode(&mut encoded);
        match result {
            Ok(ctl_opt) => {
                if let Some(packet) = ctl_opt {
                    assert_eq!(12345, packet.remaining);
                }
            }
            Err(e) => assert!(false, "error decoding remaining value: {}", e),
        }
    }
}
