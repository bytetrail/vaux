use bytes::BytesMut;
use std::io::{Read, Write};
use std::net::TcpStream;
use tokio_util::codec::{Decoder, Encoder};
use vaux_mqtt::{ConnAck, Connect, FixedHeader, MQTTCodec, Packet, PacketType, Reason};

const DEFAULT_PORT: u16 = 1883;
const DEFAULT_HOST: &'static str = "127.0.0.1";
const PING_RESP_LEN: usize = 2;
const CONNACK_RESP_LEN: usize = 2;

#[test]
/// This tests expects the basic TCP server to be running on
/// * Host: 127.0.0.1
/// * Port: 1883
fn test_basic_ping() {
    let fixed_header = FixedHeader::new(PacketType::PingResp);
    test_basic(
        Packet::PingRequest(FixedHeader::new(PacketType::PingReq)),
        PING_RESP_LEN,
        Packet::PingResponse(fixed_header),
    );
}

#[test]
fn test_basic_connect() {
    let fixed_header = FixedHeader::new(PacketType::ConnAck);
    let request = Connect::default();
    let ack = ConnAck::default();
    test_basic(
        Packet::Connect(request),
        CONNACK_RESP_LEN,
        Packet::ConnAck(ack),
    );
}

fn test_basic(request: Packet, expected_len: usize, expected_response: Packet) {
    let mut codec = MQTTCodec {};
    match TcpStream::connect((DEFAULT_HOST, DEFAULT_PORT)) {
        Ok(mut stream) => {
            let mut buffer = [0u8; 128];
            let mut dest = BytesMut::new();
            let result = codec.encode(request, &mut dest);
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
                        let result = codec.decode(&mut BytesMut::from(&buffer[0..len]));
                        match result {
                            Ok(p) => {
                                if let Some(packet) = p {
                                    assert_eq!(expected_response, packet);
                                } else {
                                    assert!(
                                        false,
                                        "expected {:?} packet type, found None",
                                        expected_response
                                    );
                                }
                            }
                            Err(e) => {
                                assert!(false, "Unexpected error decoding ping response: {:?}", e);
                            }
                        }
                    }
                    Err(e) => assert!(false, "unable to read message from broker: {:?}", e),
                },
                Err(e) => assert!(false, "unable to write message to broker: {}", e),
            }
        }
        Err(e) => assert!(false, "Unable to connect to test broker: {}", e.to_string()),
    }
}
