use bytes::BytesMut;
use std::io::{Read, Write};
use std::net::TcpStream;
use uuid::Uuid;
use vaux_mqtt::{
    decode, encode, ConnAck, Connect, Disconnect, FixedHeader, Packet, PacketType, Reason,
};

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
        Packet::Connect(Box::new(request)),
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
        Packet::Connect(Box::new(request)),
        EXPECTED_CONNACK_LEN,
        &packet,
        false,
    );
    if let Some(Packet::ConnAck(ack)) = result {
        assert!(ack
            .properties()
            .has_property(&vaux_mqtt::PropertyType::AssignedClientId));
    }
}

#[test]
fn test_existing_session() {
    let client_id = Uuid::new_v4().to_string();
    let mut client = MQTTClient::new();
    let result = client.connect(Some(&client_id));
    assert!(result.is_some(), "expected connection acknowledge");
    let ack = result.unwrap();
    assert!(!ack
        .properties()
        .has_property(&vaux_mqtt::PropertyType::AssignedClientId));
    assert_eq!(false, ack.session_present, "expected no existing session");
    client.disconnect();
    let ack = client.connect(Some(&client_id)).unwrap();
    assert!(ack.session_present, "expected session present");
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
                                if let Some(data_read) = p.as_ref() {
                                    if deep_check {
                                        assert_eq!(expected_response, &data_read.0);
                                    } else {
                                        let packet_type: PacketType = expected_response.into();
                                        assert_eq!(packet_type, (&data_read.0).into());
                                    }
                                    Some(data_read.0.clone())
                                } else {
                                    assert!(
                                        false,
                                        "expected {:?} packet type, found None",
                                        expected_response
                                    );
                                    None
                                }
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

struct MQTTClient {
    connection: Option<TcpStream>,
}

impl MQTTClient {
    fn new() -> Self {
        MQTTClient { connection: None }
    }

    fn connect(&mut self, id: Option<&str>) -> Option<ConnAck> {
        let mut connect = Connect::default();
        if let Some(id) = id {
            connect.client_id = id.to_string();
        }
        let connect_packet = Packet::Connect(Box::new(connect));
        match TcpStream::connect((DEFAULT_HOST, DEFAULT_PORT)) {
            Ok(stream) => {
                self.connection = Some(stream);
                let mut buffer = [0u8; 128];
                let mut dest = BytesMut::default();
                let result = encode(connect_packet, &mut dest);
                if let Err(e) = result {
                    assert!(false, "Failed to encode packet: {:?}", e);
                }
                match self.connection.as_ref().unwrap().write_all(&dest.to_vec()) {
                    Ok(_) => match self.connection.as_ref().unwrap().read(&mut buffer) {
                        Ok(len) => match decode(&mut BytesMut::from(&buffer[0..len])) {
                            Ok(p) => {
                                if let Some(packet) = p {
                                    // match packet {
                                    //     Packet::ConnAck(connack) => {
                                    //         return Some(connack);
                                    //     }
                                    //     Packet::Disconnect(_disconnect) => {
                                    //         panic!("disconnect");
                                    //     }
                                    //     _ => panic!("unexpected packet returned from remote"),
                                    // }
                                } else {
                                    panic!("no packet returned");
                                }
                            }
                            Err(e) => panic!("unable to decode connect response {}", e.to_string()),
                        },
                        Err(e) => panic!("unable to read stream: {}", e.to_string()),
                    },
                    Err(e) => panic!(
                        "Unable to write packet(s) to test broker: {}",
                        e.to_string()
                    ),
                }
            }
            Err(e) => assert!(false, "Unable to connect to test broker: {}", e.to_string()),
        }
        None
    }

    fn disconnect(&mut self) {
        let disconnect = Disconnect::new(Reason::Success);
        let packet = Packet::Disconnect(disconnect);
        let mut _buffer = [0u8; 128];
        let mut dest = BytesMut::default();
        let result = encode(packet, &mut dest);
        if let Err(e) = result {
            assert!(false, "Failed to encode packet: {:?}", e);
        }
        match self.connection.as_ref().unwrap().write_all(&dest.to_vec()) {
            Ok(_) => self.connection = None,
            _ => {
                panic!("unable to disconnect successfully");
            }
        }
    }
}
