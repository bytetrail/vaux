use bytes::{Bytes, BytesMut};

use crate::codec::{self, Encode, Packet, Reason};
use crate::{
    ConnAck, Connect, Disconnect, PubAck, PubComp, PubRec, PubRel, Publish, QoSLevel, WillMessage,
};
use crate::subscribe::{SubAck, Subscribe, SubscriptionFilter};
use crate::unsubscribe::{Unsubscribe, UnsubAck};

fn encode_decode(packet: &mut Packet) -> Packet {
    let mut dest = BytesMut::new();
    packet.encode(&mut dest).expect("encode failed");
    let (decoded, _) = codec::decode(&mut dest).expect("decode failed").expect("no packet");
    decoded
}

// ──────────────────────────────────────────────────────────────
// PingReq / PingResp
// ──────────────────────────────────────────────────────────────

#[test]
fn test_pingreq_roundtrip() {
    let decoded = encode_decode(&mut Packet::PingRequest(Default::default()));
    assert!(matches!(decoded, Packet::PingRequest(_)));
}

#[test]
fn test_pingresp_roundtrip() {
    let decoded = encode_decode(&mut Packet::PingResponse(Default::default()));
    assert!(matches!(decoded, Packet::PingResponse(_)));
}

// ──────────────────────────────────────────────────────────────
// Connect
// ──────────────────────────────────────────────────────────────

#[test]
fn test_connect_minimal_roundtrip() {
    let mut connect = Connect::default();
    connect.client_id = "test-client".to_string();
    connect.keep_alive = 60;
    connect.set_clean_start(true);

    let decoded = encode_decode(&mut Packet::Connect(Box::new(connect)));
    match decoded {
        Packet::Connect(c) => {
            assert_eq!(c.client_id, "test-client");
            assert_eq!(c.keep_alive, 60);
            assert!(c.clean_start());
        }
        _ => panic!("expected Connect"),
    }
}

#[test]
fn test_connect_with_properties() {
    let mut connect = Connect::default();
    connect.client_id = "props-client".to_string();
    connect.session_expiry_interval = Some(300);
    connect.receive_maximum = Some(100);
    connect.max_packet_size = Some(65536);
    connect.topic_alias_maximum = Some(10);
    connect.request_response_info = Some(true);
    connect.request_problem_info = Some(false);

    let decoded = encode_decode(&mut Packet::Connect(Box::new(connect)));
    match decoded {
        Packet::Connect(c) => {
            assert_eq!(c.session_expiry_interval, Some(300));
            assert_eq!(c.receive_maximum, Some(100));
            assert_eq!(c.max_packet_size, Some(65536));
            assert_eq!(c.topic_alias_maximum, Some(10));
            assert_eq!(c.request_response_info, Some(true));
            assert_eq!(c.request_problem_info, Some(false));
        }
        _ => panic!("expected Connect"),
    }
}

#[test]
fn test_connect_with_credentials() {
    let mut connect = Connect::default();
    connect.client_id = "auth-client".to_string();
    connect.set_username(Some("user".to_string()));
    connect.set_password(Some(vec![1, 2, 3, 4]));

    let decoded = encode_decode(&mut Packet::Connect(Box::new(connect)));
    match decoded {
        Packet::Connect(c) => {
            assert!(c.username());
            assert!(c.password());
        }
        _ => panic!("expected Connect"),
    }
}

#[test]
fn test_connect_with_will() {
    let mut connect = Connect::default();
    connect.client_id = "will-client".to_string();
    let will = WillMessage::new("will/topic".to_string(), b"goodbye", QoSLevel::AtLeastOnce, true);
    connect.set_will_message(will);

    let decoded = encode_decode(&mut Packet::Connect(Box::new(connect)));
    match decoded {
        Packet::Connect(c) => {
            assert!(c.will());
            assert!(c.will_retain());
            assert_eq!(c.will_qos().unwrap(), QoSLevel::AtLeastOnce);
            let wm = c.will_message().expect("expected will message");
            assert_eq!(wm.topic, "will/topic");
            assert_eq!(wm.payload, Bytes::from_static(b"goodbye"));
        }
        _ => panic!("expected Connect"),
    }
}

// ──────────────────────────────────────────────────────────────
// ConnAck
// ──────────────────────────────────────────────────────────────

#[test]
fn test_connack_minimal_roundtrip() {
    let connack = ConnAck::default();
    let decoded = encode_decode(&mut Packet::ConnAck(connack));
    match decoded {
        Packet::ConnAck(c) => {
            assert_eq!(c.reason, Reason::Success);
            assert!(!c.session_present());
        }
        _ => panic!("expected ConnAck"),
    }
}

#[test]
fn test_connack_with_properties() {
    let mut connack = ConnAck::default();
    connack.set_session_present(true);
    connack.reason = Reason::Success;
    connack.set_session_expiry(600);
    connack.set_receive_max(50);
    connack.set_server_keep_alive(30);
    connack.maximum_packet_size = Some(1048576);
    connack.topic_alias_maximum = Some(20);
    connack.assigned_client_id = Some("server-assigned-id".to_string());
    connack.reason_string = Some("all good".to_string());

    let decoded = encode_decode(&mut Packet::ConnAck(connack));
    match decoded {
        Packet::ConnAck(c) => {
            assert!(c.session_present());
            assert_eq!(c.session_expiry_interval, Some(600));
            assert_eq!(c.receive_max(), Some(50));
            assert_eq!(c.server_keep_alive, Some(30));
            assert_eq!(c.maximum_packet_size, Some(1048576));
            assert_eq!(c.topic_alias_maximum, Some(20));
            assert_eq!(c.assigned_client_id, Some("server-assigned-id".to_string()));
            assert_eq!(c.reason_string, Some("all good".to_string()));
        }
        _ => panic!("expected ConnAck"),
    }
}

// ──────────────────────────────────────────────────────────────
// Publish
// ──────────────────────────────────────────────────────────────

#[test]
fn test_publish_qos0_roundtrip() {
    let mut publish = Publish::new_with_message(
        None,
        "test/topic".to_string(),
        QoSLevel::AtMostOnce,
        "hello mqtt",
    )
    .unwrap();
    publish.set_retain(true);

    let decoded = encode_decode(&mut Packet::Publish(publish));
    match decoded {
        Packet::Publish(p) => {
            assert_eq!(p.topic_name, "test/topic");
            assert_eq!(p.payload, Some(Bytes::from_static(b"hello mqtt")));
            assert_eq!(p.qos(), QoSLevel::AtMostOnce);
            assert!(p.retain());
            assert_eq!(p.packet_id(), None);
        }
        _ => panic!("expected Publish"),
    }
}

#[test]
fn test_publish_qos1_roundtrip() {
    let publish = Publish::new_with_payload(
        Some(42),
        "sensor/temp".to_string(),
        QoSLevel::AtLeastOnce,
        vec![0xDE, 0xAD, 0xBE, 0xEF],
    )
    .unwrap();

    let decoded = encode_decode(&mut Packet::Publish(publish));
    match decoded {
        Packet::Publish(p) => {
            assert_eq!(p.topic_name, "sensor/temp");
            assert_eq!(p.packet_id(), Some(42));
            assert_eq!(p.qos(), QoSLevel::AtLeastOnce);
            assert_eq!(p.payload, Some(Bytes::from_static(&[0xDE, 0xAD, 0xBE, 0xEF])));
        }
        _ => panic!("expected Publish"),
    }
}

#[test]
fn test_publish_with_properties() {
    let mut publish = Publish::new_with_message(
        Some(100),
        "props/topic".to_string(),
        QoSLevel::ExactlyOnce,
        "payload",
    )
    .unwrap();
    publish.message_expiry = Some(3600);
    publish.content_type = Some("text/plain".to_string());
    publish.response_topic = Some("reply/topic".to_string());
    publish = publish.with_topic_alias(5);

    let decoded = encode_decode(&mut Packet::Publish(publish));
    match decoded {
        Packet::Publish(p) => {
            assert_eq!(p.message_expiry, Some(3600));
            assert_eq!(p.content_type, Some("text/plain".to_string()));
            assert_eq!(p.response_topic, Some("reply/topic".to_string()));
            assert_eq!(p.topic_alias, Some(5));
            assert_eq!(p.qos(), QoSLevel::ExactlyOnce);
        }
        _ => panic!("expected Publish"),
    }
}

// ──────────────────────────────────────────────────────────────
// PubAck
// ──────────────────────────────────────────────────────────────

#[test]
fn test_puback_success_abbreviated() {
    let puback = PubAck::new_puback_with_packet_id(1);
    let decoded = encode_decode(&mut Packet::PubAck(puback));
    match decoded {
        Packet::PubAck(p) => {
            assert_eq!(p.packet_id, 1);
            assert_eq!(p.reason(), None);
        }
        _ => panic!("expected PubAck"),
    }
}

#[test]
fn test_puback_with_reason() {
    let mut puback = PubAck::new_puback_with_packet_id(99);
    puback.set_reason(Reason::NoSubscribers).unwrap();

    let decoded = encode_decode(&mut Packet::PubAck(puback));
    match decoded {
        Packet::PubAck(p) => {
            assert_eq!(p.packet_id, 99);
            assert_eq!(p.reason(), Some(Reason::NoSubscribers));
        }
        _ => panic!("expected PubAck"),
    }
}

// ──────────────────────────────────────────────────────────────
// PubRec
// ──────────────────────────────────────────────────────────────

#[test]
fn test_pubrec_success_abbreviated() {
    let pubrec = PubRec::new_pubrec_with_packet_id(10);
    let decoded = encode_decode(&mut Packet::PubRec(pubrec));
    match decoded {
        Packet::PubRec(p) => {
            assert_eq!(p.packet_id, 10);
            assert_eq!(p.reason(), None);
        }
        _ => panic!("expected PubRec"),
    }
}

#[test]
fn test_pubrec_with_reason() {
    let mut pubrec = PubRec::new_pubrec_with_packet_id(200);
    pubrec.set_reason(Reason::QuotaExceeded).unwrap();

    let decoded = encode_decode(&mut Packet::PubRec(pubrec));
    match decoded {
        Packet::PubRec(p) => {
            assert_eq!(p.packet_id, 200);
            assert_eq!(p.reason(), Some(Reason::QuotaExceeded));
        }
        _ => panic!("expected PubRec"),
    }
}

// ──────────────────────────────────────────────────────────────
// PubRel
// ──────────────────────────────────────────────────────────────

#[test]
fn test_pubrel_success_abbreviated() {
    let pubrel = PubRel::new_pubrel_with_packet_id(7);
    let decoded = encode_decode(&mut Packet::PubRel(pubrel));
    match decoded {
        Packet::PubRel(p) => {
            assert_eq!(p.packet_id, 7);
            assert_eq!(p.reason(), None);
        }
        _ => panic!("expected PubRel"),
    }
}

#[test]
fn test_pubrel_with_reason() {
    let mut pubrel = PubRel::new_pubrel_with_packet_id(55);
    pubrel.set_reason(Reason::PacketIdInUse).unwrap();

    let decoded = encode_decode(&mut Packet::PubRel(pubrel));
    match decoded {
        Packet::PubRel(p) => {
            assert_eq!(p.packet_id, 55);
            assert_eq!(p.reason(), Some(Reason::PacketIdInUse));
        }
        _ => panic!("expected PubRel"),
    }
}

// ──────────────────────────────────────────────────────────────
// PubComp
// ──────────────────────────────────────────────────────────────

#[test]
fn test_pubcomp_success_abbreviated() {
    let pubcomp = PubComp::new_pubcomp_with_packet_id(33);
    let decoded = encode_decode(&mut Packet::PubComp(pubcomp));
    match decoded {
        Packet::PubComp(p) => {
            assert_eq!(p.packet_id, 33);
            assert_eq!(p.reason(), None);
        }
        _ => panic!("expected PubComp"),
    }
}

#[test]
fn test_pubcomp_with_reason() {
    let mut pubcomp = PubComp::new_pubcomp_with_packet_id(44);
    pubcomp.set_reason(Reason::PacketIdInUse).unwrap();

    let decoded = encode_decode(&mut Packet::PubComp(pubcomp));
    match decoded {
        Packet::PubComp(p) => {
            assert_eq!(p.packet_id, 44);
            assert_eq!(p.reason(), Some(Reason::PacketIdInUse));
        }
        _ => panic!("expected PubComp"),
    }
}

// ──────────────────────────────────────────────────────────────
// Subscribe
// ──────────────────────────────────────────────────────────────

#[test]
fn test_subscribe_single_filter() {
    let mut sub = Subscribe::new_with_packet_id(1);
    sub.add_filter(SubscriptionFilter::new("test/topic".to_string(), QoSLevel::AtLeastOnce));

    let decoded = encode_decode(&mut Packet::Subscribe(sub));
    match decoded {
        Packet::Subscribe(s) => {
            assert_eq!(s.packet_id, 1);
            assert_eq!(s.filter.len(), 1);
            assert_eq!(s.filter[0].filter, "test/topic");
            assert_eq!(s.filter[0].qos(), QoSLevel::AtLeastOnce);
        }
        _ => panic!("expected Subscribe"),
    }
}

#[test]
fn test_subscribe_multiple_filters() {
    let filters = vec![
        SubscriptionFilter::new("sensor/#".to_string(), QoSLevel::ExactlyOnce),
        SubscriptionFilter::new("cmd/+/exec".to_string(), QoSLevel::AtMostOnce),
    ];
    let sub = Subscribe::new_with_filter(5, filters);

    let decoded = encode_decode(&mut Packet::Subscribe(sub));
    match decoded {
        Packet::Subscribe(s) => {
            assert_eq!(s.packet_id, 5);
            assert_eq!(s.filter.len(), 2);
            assert_eq!(s.filter[0].filter, "sensor/#");
            assert_eq!(s.filter[0].qos(), QoSLevel::ExactlyOnce);
            assert_eq!(s.filter[1].filter, "cmd/+/exec");
            assert_eq!(s.filter[1].qos(), QoSLevel::AtMostOnce);
        }
        _ => panic!("expected Subscribe"),
    }
}

// ──────────────────────────────────────────────────────────────
// SubAck
// ──────────────────────────────────────────────────────────────

#[test]
fn test_suback_roundtrip() {
    let mut suback = SubAck::new_with_packet_id(3).unwrap();
    suback.reason_codes.push(Reason::GrantedQoS1);
    suback.reason_codes.push(Reason::GrantedQoS2);

    let decoded = encode_decode(&mut Packet::SubAck(suback));
    match decoded {
        Packet::SubAck(s) => {
            assert_eq!(s.packet_id(), 3);
            assert_eq!(s.reason_codes.len(), 2);
            assert_eq!(s.reason_codes[0], Reason::GrantedQoS1);
            assert_eq!(s.reason_codes[1], Reason::GrantedQoS2);
        }
        _ => panic!("expected SubAck"),
    }
}

// ──────────────────────────────────────────────────────────────
// Unsubscribe
// ──────────────────────────────────────────────────────────────

#[test]
fn test_unsubscribe_roundtrip() {
    let unsub = Unsubscribe::new(
        7,
        vec!["topic/a".to_string(), "topic/b".to_string()],
    );

    let decoded = encode_decode(&mut Packet::Unsubscribe(unsub));
    match decoded {
        Packet::Unsubscribe(u) => {
            assert_eq!(u.packet_id, 7);
            assert_eq!(u.topics.len(), 2);
            assert_eq!(u.topics[0], "topic/a");
            assert_eq!(u.topics[1], "topic/b");
        }
        _ => panic!("expected Unsubscribe"),
    }
}

// ──────────────────────────────────────────────────────────────
// UnsubAck
// ──────────────────────────────────────────────────────────────

#[test]
fn test_unsuback_roundtrip() {
    let mut unsuback = UnsubAck::default();
    unsuback.packet_id = 9;
    unsuback.reason_code.push(Reason::Success);
    unsuback.reason_code.push(Reason::NoSubscribers);

    let decoded = encode_decode(&mut Packet::UnsubAck(unsuback));
    match decoded {
        Packet::UnsubAck(u) => {
            assert_eq!(u.packet_id, 9);
            assert_eq!(u.reason_code.len(), 2);
            assert_eq!(u.reason_code[0], Reason::Success);
            assert_eq!(u.reason_code[1], Reason::NoSubscribers);
        }
        _ => panic!("expected UnsubAck"),
    }
}

// ──────────────────────────────────────────────────────────────
// Disconnect
// ──────────────────────────────────────────────────────────────

#[test]
fn test_disconnect_normal_abbreviated() {
    let disconnect = Disconnect::default();
    let mut dest = BytesMut::new();
    Packet::Disconnect(disconnect.clone()).encode(&mut dest).unwrap();
    assert_eq!(dest.len(), 2, "abbreviated disconnect should be 2 bytes (header + remaining 0)");

    let decoded = encode_decode(&mut Packet::Disconnect(disconnect));
    match decoded {
        Packet::Disconnect(d) => {
            assert_eq!(d.reason, Reason::NormalDisconnect);
        }
        _ => panic!("expected Disconnect"),
    }
}

#[test]
fn test_disconnect_with_reason() {
    let disconnect = Disconnect::new(Reason::ServerShutdown);
    let decoded = encode_decode(&mut Packet::Disconnect(disconnect));
    match decoded {
        Packet::Disconnect(d) => {
            assert_eq!(d.reason, Reason::ServerShutdown);
        }
        _ => panic!("expected Disconnect"),
    }
}

#[test]
fn test_disconnect_with_properties() {
    let mut disconnect = Disconnect::new(Reason::NormalDisconnect);
    disconnect.session_expiry_interval = Some(120);
    disconnect.reason_string = Some("closing".to_string());
    disconnect.server_reference = Some("other-server:1883".to_string());

    let decoded = encode_decode(&mut Packet::Disconnect(disconnect));
    match decoded {
        Packet::Disconnect(d) => {
            assert_eq!(d.reason, Reason::NormalDisconnect);
            assert_eq!(d.session_expiry_interval, Some(120));
            assert_eq!(d.reason_string, Some("closing".to_string()));
            assert_eq!(d.server_reference, Some("other-server:1883".to_string()));
        }
        _ => panic!("expected Disconnect"),
    }
}

// ──────────────────────────────────────────────────────────────
// User Properties (cross-cutting, tested on ConnAck)
// ──────────────────────────────────────────────────────────────

#[test]
fn test_connack_with_user_properties() {
    let mut connack = ConnAck::default();
    connack.user_properties.add("key1".to_string(), "value1".to_string());
    connack.user_properties.add("key2".to_string(), "value2".to_string());

    let decoded = encode_decode(&mut Packet::ConnAck(connack));
    match decoded {
        Packet::ConnAck(c) => {
            assert_eq!(c.user_properties.get("key1"), Some(&vec!["value1".to_string()]));
            assert_eq!(c.user_properties.get("key2"), Some(&vec!["value2".to_string()]));
        }
        _ => panic!("expected ConnAck"),
    }
}
