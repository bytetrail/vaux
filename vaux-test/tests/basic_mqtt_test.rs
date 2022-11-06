use bytes::BytesMut;
use std::io::{Read, Write};
use std::net::TcpStream;
use uuid::Uuid;
use vaux_mqtt::{decode, encode, ConnAck, Connect, FixedHeader, Packet, PacketType};

const DEFAULT_PORT: u16 = 1883;
const DEFAULT_HOST: &'static str = "127.0.0.1";
const PING_RESP_LEN: usize = 2;
const CONNACK_RESP_LEN: usize = 5;

#[test]
fn test_basic_ping() {
    let fixed_header = FixedHeader::new(PacketType::PingResp);
    test_basic(
        Packet::PingRequest(FixedHeader::new(PacketType::PingReq)),
        PING_RESP_LEN,
        &Packet::PingResponse(fixed_header),
        true,
    );
}

#[test]
fn test_basic_connect() {
    let mut request = Connect::default();
    request.client_id = Uuid::new_v4().to_string();
    let ack = ConnAck::default();
    test_basic(
        Packet::Connect(request),
        CONNACK_RESP_LEN,
        &Packet::ConnAck(ack),
        true,
    );
}

#[test]
fn test_broker_assigned_id() {
    const EXPECTED_CONNACK_LEN: usize = 44;
    let request = Connect::default();
    let ack = ConnAck::default();
    let packet = Packet::ConnAck(ack);
    let result = test_basic(
        Packet::Connect(request),
        EXPECTED_CONNACK_LEN,
        &packet,
        false,
    );
    if let Some(Packet::ConnAck(ack)) = result {
        assert!(ack.assigned_client_id.is_some());
    }
}

fn test_basic(
    request: Packet,
    expected_len: usize,
    expected_response: &Packet,
    deep_check: bool,
) -> Option<Packet> {
    match TcpStream::connect((DEFAULT_HOST, DEFAULT_PORT)) {
        Ok(mut stream) => {
            let mut buffer = [0u8; 128];
            let mut dest = BytesMut::new();
            let result = encode(request.clone(), &mut dest);
            if let Err(e) = result {
                assert!(false, "Failed to encode packet: {:?}", e);
            }
            match stream.write_all(&dest.to_vec()) {
                Ok(_) => match stream.read(&mut buffer) {
                    Ok(len) => {
                        assert_eq!(
                            expected_len, len,
                            "expected {} bytes in response",
                            expected_len
                        );
                        let result = decode(&mut BytesMut::from(&buffer[0..len]));
                        match result {
                            Ok(p) => {
                                if let Some(packet) = p.as_ref() {
                                    if deep_check {
                                        assert_eq!(expected_response, packet);
                                    } else {
                                        let packet_type: PacketType = expected_response.into();
                                        assert_eq!(packet_type, packet.into());
                                    }
                                } else {
                                    assert!(
                                        false,
                                        "expected {:?} packet type, found None",
                                        expected_response
                                    );
                                }
                                p
                            }
                            Err(e) => {
                                assert!(false, "Unexpected error decoding ping response: {:?}", e);
                                None
                            }
                        }
                    }
                    Err(e) => {
                        assert!(false, "unable to read message from broker: {:?}", e);
                        None
                    }
                },
                Err(e) => {
                    assert!(false, "unable to write message to broker: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            assert!(false, "Unable to connect to test broker: {}", e.to_string());
            None
        }
    }
}
