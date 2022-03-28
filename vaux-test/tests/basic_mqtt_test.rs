use bytes::BytesMut;
use std::io::{Read, Write};
use std::net::TcpStream;
use tokio_util::codec::{Decoder, Encoder};
use vaux_mqtt::{ControlPacket, MQTTCodec, PacketType};

const DEFAULT_PORT: u16 = 1883;
const DEFAULT_HOST: &'static str = "127.0.0.1";
const PING_RESP_LEN: usize = 2;

#[test]
/// This tests expects the basic TCP server to be running on
/// * Host: 127.0.0.1
/// * Port: 1883
fn test_ping() {
    let mut codec = MQTTCodec {};
    match TcpStream::connect((DEFAULT_HOST, DEFAULT_PORT)) {
        Ok(mut stream) => {
            let mut buffer = [0u8; 128];
            let ping = ControlPacket::new(PacketType::PingReq);
            let mut dest = BytesMut::new();
            let result = codec.encode(ping, &mut dest);

            match stream.write_all(&dest.to_vec()) {
                Ok(_) => match stream.read(&mut buffer) {
                    Ok(len) => {
                        assert_eq!(
                            PING_RESP_LEN, len,
                            "expected {} bytes in response",
                            PING_RESP_LEN
                        );
                        let result = codec.decode(&mut BytesMut::from(&buffer[0..len]));
                        match result {
                            Ok(p) => {
                                if let Some(packet) = p {
                                    assert_eq!(PacketType::PingResp, packet.packet_type());
                                } else {
                                    assert!(false, "expected PingResp packet type, found None");
                                }
                            }
                            Err(e) => {
                                assert!(false, "Unexpected error decoding ping response: {:?}", e);
                            }
                        }
                    }
                    Err(e) => assert!(false, "unable to read echo message from broker: {:?}", e),
                },
                Err(e) => assert!(false, "unable to write echo message to broker: {}", e),
            }
        }
        Err(e) => assert!(false, "Unable to connect to test broker: {}", e.to_string()),
    }
}
