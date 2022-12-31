use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
};

use bytes::BytesMut;
use vaux_mqtt::{decode, encode, Connect, Packet};

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 1883;

pub struct MQTTClient {
    addr: SocketAddr,
    client_id: Option<String>,
    connection: Option<TcpStream>,
}

#[derive(Debug)]
pub enum ErrorKind {
    Codec,
    Protocol,
    Connection,
    Transport,
}

#[derive(Debug)]
pub struct MQTTError {
    message: String,
    kind: ErrorKind,
}

impl MQTTError {
    pub fn new(message: &str, kind: ErrorKind) -> Self {
        Self {
            message: message.to_string(),
            kind,
        }
    }
}

pub type MQTTResult<T> = Result<T, MQTTError>;

impl MQTTClient {
    pub fn new(addr: SocketAddr) -> Self {
        MQTTClient {
            addr,
            client_id: None,
            connection: None,
        }
    }

    pub fn new_with_id(addr: SocketAddr, id: String) -> Self {
        MQTTClient {
            addr,
            client_id: Some(id),
            connection: None,
        }
    }

    pub fn connect(&mut self) -> MQTTResult<()> {
        let connect = Connect::default();
        let connect_packet = Packet::Connect(connect);
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
                                    match packet {
                                        Packet::ConnAck(connack) => {
                                            if self.client_id.is_none() {
                                                self.client_id = connack.assigned_client_id;
                                            }
                                            // TODO set server properties based on ConnAck
                                            return Ok(());
                                        }
                                        Packet::Disconnect(_disconnect) => {
                                            // TODO return the disconnect reason as MQTT error
                                            panic!("disconnect");
                                        }
                                        _ => {
                                            return Err(MQTTError::new(
                                                "unexpected packet type",
                                                ErrorKind::Protocol,
                                            ))
                                        }
                                    }
                                } else {
                                    return Err(MQTTError::new(
                                        "no MQTT packet received",
                                        ErrorKind::Protocol,
                                    ));
                                }
                            }
                            Err(e) => return Err(MQTTError::new(&e.to_string(), ErrorKind::Codec)),
                        },
                        Err(e) => {
                            return Err(MQTTError::new(
                                &format!("unable to read stream: {}", e.to_string()),
                                ErrorKind::Transport,
                            ))
                        }
                    },
                    Err(e) => panic!(
                        "Unable to write packet(s) to test broker: {}",
                        e.to_string()
                    ),
                }
            }
            Err(e) => {
                return Err(MQTTError::new(
                    &format!("unable to connect: {}", e.to_string()),
                    ErrorKind::Connection,
                ))
            }
        }
    }

    pub fn send(topic: &str, message: &[u8]) -> MQTTResult<()> {
        Ok(())
    }

    pub fn disconnect(&mut self) {
        if let Some(_conn) = self.connection.as_mut() {
        } else {
            // it is an error to disconnect when not connected
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
