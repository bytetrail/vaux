use bytes::{BufMut, BytesMut};

use crate::codec::*;

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
    put_var_u32(test, &mut encoded);
    assert_eq!(0x80, encoded[0]);
    assert_eq!(0x01, encoded[1]);
    let test = 777;
    let mut encoded: BytesMut = BytesMut::with_capacity(6);
    put_var_u32(test, &mut encoded);
    assert_eq!(0x89, encoded[0]);
    assert_eq!(0x06, encoded[1]);
}

#[test]
fn test_decode_var_int() {
    let mut encoded: BytesMut = BytesMut::with_capacity(6);
    // 0x80
    encoded.put_u8(0x80);
    encoded.put_u8(0x01);
    let val = get_var_u32(&mut encoded).unwrap();
    assert_eq!(128, val);
    // 777 --- 0x309
    encoded.clear();
    encoded.put_u8(0x89);
    encoded.put_u8(0x06);
    let val = get_var_u32(&mut encoded).unwrap();
    assert_eq!(777, val);
}
