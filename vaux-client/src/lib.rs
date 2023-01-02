use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

use bytes::BytesMut;
use vaux_mqtt::{decode, encode, publish::Publish, Connect, Packet, QoSLevel};

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

    pub fn new_with_id(addr: SocketAddr, id: &str) -> Self {
        MQTTClient {
            addr,
            client_id: Some(id.to_owned()),
            connection: None,
        }
    }

    pub fn connect(&mut self) -> MQTTResult<()> {
        let mut connect = Connect::default();
        if let Some(id) = self.client_id.as_ref() {
            connect.client_id = id.to_owned();
        }
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

    /// Basic send of a UTF8 encoded payload. The message is sent with QoS
    /// Level 0 and a payload format indicated a UTF8. No additional properties
    /// are set. A disconnect message can occur due to errors. This method uses
    /// a simple blocking read with a timeout for test purposes.
    pub fn send(&mut self, topic: &str, message: &str) -> MQTTResult<()> {
        let mut publish = Publish::default();
        publish.payload_utf8 = true;
        publish.topic_name = Some(topic.to_string());
        publish.payload = Some(Vec::from(message.as_bytes()));

        let mut buffer = [0u8; 128];
        let mut dest = BytesMut::default();
        let result = encode(Packet::Publish(publish.clone()), &mut dest);
        if let Err(e) = result {
            assert!(false, "Failed to encode packet: {:?}", e);
        }
        match self.connection.as_ref().unwrap().write_all(&dest.to_vec()) {
            Ok(_) => {
                // set minimal timeout for potential disconnect on publish when
                // no other packet expected
                if self
                    .connection
                    .as_mut()
                    .unwrap()
                    .set_read_timeout(Some(Duration::from_millis(200)))
                    .is_err()
                {
                    return Err(MQTTError::new(
                        "unable to set connection read timeout",
                        ErrorKind::Transport,
                    ));
                }
                match self.connection.as_ref().unwrap().read(&mut buffer) {
                    Ok(len) => match decode(&mut BytesMut::from(&buffer[0..len])) {
                        Ok(packet) => {
                            if let Some(Packet::Disconnect(d)) = packet {
                                eprintln!("unexpected disconnect due to {}", d.reason);
                                if let Some(desc) = d.reason_desc {
                                    eprintln!("description: {}", desc);
                                }
                            }
                        }
                        Err(e) => eprintln!("{:#?}", e),
                    },
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::TimedOut {
                            return Ok(());
                        }
                        else {
                            eprintln!("unexpected transport error {:#?}", e);
                        }
                    }
                }
            }
            Err(e) => eprintln!("unexpected send error {:#?}", e),
        }
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
