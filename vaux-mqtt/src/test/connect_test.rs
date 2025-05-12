use bytes::BytesMut;

use crate::property::PacketProperties;
#[allow(unused_imports)]
use crate::{connect::*, property::Property};
#[allow(unused_imports)]
use crate::{property::PropertySize, Encode, PropertyType, QoSLevel, Size, WillMessage};

#[allow(dead_code)]
const CONNECT_MIN_REMAINING: u32 = 13;
#[allow(dead_code)]
const PROP_ENCODE: u32 = 5;

#[test]
fn test_encode_flags() {
    let mut connect = Connect::default();
    connect.clean_start = true;
    let mut dest = BytesMut::new();
    let result = connect.encode(&mut dest);
    assert!(result.is_ok());
    assert_eq!(dest[9], CONNECT_FLAG_CLEAN_START);
    let mut dest = BytesMut::new();
    let password = vec![1, 2, 3, 4, 5];
    connect.password = Some(password);
    let result = connect.encode(&mut dest);
    assert!(result.is_ok());
    assert_eq!(dest[9], CONNECT_FLAG_CLEAN_START | CONNECT_FLAG_PASSWORD);
}

#[test]
fn test_encode_will() {
    let mut connect = Connect::default();
    let will = WillMessage::new(QoSLevel::AtLeastOnce, true);
    connect.will_message = Some(will);
    let mut dest = BytesMut::new();
    let result = connect.encode(&mut dest);
    assert!(result.is_ok());
    assert!(dest[9] & CONNECT_FLAG_WILL > 0);
    assert!(dest[9] & CONNECT_FLAG_WILL_RETAIN > 0);
    let qos_bits = (dest[9] & CONNECT_FLAG_WILL_QOS) >> CONNECT_FLAG_SHIFT;
    let qos_result = QoSLevel::try_from(qos_bits);
    assert!(qos_result.is_ok());
    let qos = qos_result.unwrap();
    assert_eq!(QoSLevel::AtLeastOnce, qos);
}

#[test]
fn test_encode_keep_alive() {
    let mut connect = Connect::default();
    connect.keep_alive = 0xcafe;
    let mut dest = BytesMut::new();
    let result = connect.encode(&mut dest);
    assert!(result.is_ok());
    assert_eq!(0xcafe, ((dest[10] as u16) << 8) + dest[11] as u16);
}

#[test]
fn test_encode_session_expiry() {
    let mut connect = Connect::default();
    connect
        .properties_mut()
        .set_property(Property::SessionExpiryInterval(0xcafe));
    let mut dest = BytesMut::new();
    test_property(connect, &mut dest, 5, PropertyType::SessionExpiryInterval);
}

#[test]
fn test_encode_receive_max() {
    let mut connect = Connect::default();
    connect
        .properties_mut()
        .set_property(Property::RecvMax(0xcafe));
    let mut dest = BytesMut::new();
    test_property(connect, &mut dest, 3, PropertyType::RecvMax);
}

#[test]
fn test_encode_max_packet_size() {
    let mut connect = Connect::default();
    connect
        .properties_mut()
        .set_property(Property::MaxPacketSize(0xcafe));
    let mut dest = BytesMut::new();
    test_property(connect, &mut dest, 5, PropertyType::MaxPacketSize);
}

#[test]
fn test_encode_topic_alias_max() {
    let mut connect = Connect::default();
    connect
        .properties_mut()
        .set_property(Property::TopicAliasMax(0xcafe));
    let mut dest = BytesMut::new();
    test_property(connect, &mut dest, 3, PropertyType::TopicAliasMax);
}

#[test]
fn test_encode_req_resp_info() {
    let mut connect = Connect::default();
    connect
        .properties_mut()
        .set_property(Property::ReqRespInfo(true));
    let mut dest = BytesMut::new();
    test_property(connect, &mut dest, 2, PropertyType::ReqRespInfo);
}

#[test]
fn test_encode_req_problem_info() {
    let mut connect = Connect::default();
    connect
        .properties_mut()
        .set_property(Property::ReqProblemInfo(true));
    let mut dest = BytesMut::new();
    test_property(connect, &mut dest, 2, PropertyType::ReqProblemInfo);
}

#[test]
fn test_encode_auth_method() {
    let mut connect = Connect::default();
    connect
        .properties_mut()
        .set_property(Property::AuthMethod("123-456-789".to_string()));
    let mut dest = BytesMut::new();
    test_property(connect, &mut dest, 14, PropertyType::AuthMethod);
}

#[test]
fn test_encode_auth_data() {
    let mut connect = Connect::default();
    connect
        .properties_mut()
        .set_property(Property::AuthData(vec![1, 2, 3, 4, 5]));
    let mut dest = BytesMut::new();
    test_property(connect, &mut dest, 8, PropertyType::AuthData);
}

#[test]
fn test_encode_client_id() {
    let mut connect = Connect::default();
    connect.client_id = "123-456-789".to_string();
    let mut dest = BytesMut::new();
    let result = connect.encode(&mut dest);
    assert!(result.is_ok());
    assert_eq!(26, dest.len() as u32, "Packet Size");
    assert_eq!('1', dest[15] as char)
}

#[test]
fn test_default_remaining() {
    let connect = Connect::default();
    let remaining = connect.size();
    assert_eq!(
        CONNECT_MIN_REMAINING, remaining,
        "[Default] expected {} remaining size",
        CONNECT_MIN_REMAINING
    );
    let mut dest = BytesMut::new();
    let result = connect.encode(&mut dest);
    assert!(result.is_ok());
    assert_eq!(
        CONNECT_MIN_REMAINING as usize,
        dest.len() - 2,
        "Expected minimum size {}",
        CONNECT_MIN_REMAINING
    );
}

#[test]
fn test_receive_max_remaining() {
    let mut connect = Connect::default();
    connect
        .properties_mut()
        .set_property(Property::RecvMax(1024));
    let remaining = connect.size();
    let expected = CONNECT_MIN_REMAINING + 3;
    assert_eq!(
        expected, remaining,
        "[Receive Max] expected {} remaining size",
        expected
    );
}

#[test]
fn test_problem_info_remaining() {
    let mut connect = Connect::default();
    connect
        .properties_mut()
        .set_property(Property::ReqProblemInfo(false));
    let remaining = connect.size();
    let expected = CONNECT_MIN_REMAINING + 2;
    assert_eq!(
        expected, remaining,
        "[Problem Info false] expected {} remaining size",
        expected
    );
    connect
        .properties_mut()
        .clear_property(PropertyType::ReqProblemInfo);
    let remaining = connect.size();
    assert_eq!(
        CONNECT_MIN_REMAINING, remaining,
        "[Problem Info true] {} remaining size",
        CONNECT_MIN_REMAINING
    );
}

#[test]
fn test_user_property_remaining() {
    let mut connect = Connect::default();

    let key = "12335";
    let value = "12345";
    let expected = CONNECT_MIN_REMAINING + key.len() as u32 + value.len() as u32 + PROP_ENCODE;
    connect
        .properties_mut()
        .add_user_property(key.to_string(), value.to_string());
    let remaining = connect.size();
    assert_eq!(
        expected, remaining,
        "[Single Property] expected {} remaining size",
        expected
    );
    connect.properties_mut().clear();
    let key = "12335".to_string();
    let value = "12345".to_string();
    let mut expected = CONNECT_MIN_REMAINING + key.len() as u32 + value.len() as u32 + PROP_ENCODE;
    connect.properties_mut().add_user_property(key, value);
    let key = "567890".to_string();
    let value = "567890".to_string();
    expected += key.len() as u32 + value.len() as u32 + PROP_ENCODE;
    connect.properties_mut().add_user_property(key, value);
    let remaining = connect.size();
    assert_eq!(
        expected, remaining,
        "[2 Properties] expected {} remaining size",
        expected
    );
}

/// Remaining size calculations for a connect packet with will message present.
#[test]
fn test_will_message_remaining() {
    let mut connect = Connect::default();
    let will_message = WillMessage::new(QoSLevel::AtLeastOnce, true);
    connect.will_message = Some(will_message);
    let remaining = connect.size();
    assert_eq!(
        CONNECT_MIN_REMAINING + 5,
        remaining,
        "[Min Will Message] expected {}",
        CONNECT_MIN_REMAINING + 5
    );
}

#[allow(dead_code)]
fn test_property(
    connect: Connect,
    dest: &mut BytesMut,
    expected_prop_len: u32,
    property: PropertyType,
) {
    let expected_len = 15 + expected_prop_len;
    let result = connect.encode(dest);
    assert!(result.is_ok());
    assert_eq!(expected_len, dest.len() as u32, "Packet Size");
    assert_eq!(
        expected_prop_len,
        connect.property_size(),
        "Encoded Property Length"
    );
    assert_eq!(expected_prop_len as u8, dest[12], "Property Length");
    assert_eq!(property as u8, dest[13], "Property Type");
}
