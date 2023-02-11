use std::{
    io::{Read, Write},
    net::{IpAddr, TcpStream},
    time::Duration, default,
};

use bytes::BytesMut;
use vaux_mqtt::{
    decode, encode, publish::Publish, Subscription, Connect, Packet, Subscribe, UserPropertyMap,
};

pub struct MQTTClient {
    addr: IpAddr,
    port: u16,
    client_id: Option<String>,
    connection: Option<TcpStream>,
}

#[derive(Default,Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorKind {
    #[default]
    Codec,
    Protocol,
    Connection,
    Timeout,
    Transport,
}

#[derive(Default,Debug)]
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

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

pub type MQTTResult<T> = Result<T, MQTTError>;

impl MQTTClient {
    pub fn new(addr: IpAddr, port: u16) -> Self {
        MQTTClient {
            addr,
            port,
            client_id: None,
            connection: None,
        }
    }

    pub fn new_with_id(addr: IpAddr, port: u16, id: &str) -> Self {
        MQTTClient {
            addr,
            port,
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
        match TcpStream::connect((self.addr, self.port)) {
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

    pub fn send_binary(&mut self, topic: &str, data: &[u8], props: Option<&UserPropertyMap>) -> MQTTResult<Option<Packet>> {
        self.publish(topic, false, data, props)
    }

    pub fn send_utf8(&mut self, topic: &str, message: &str, props: Option<&UserPropertyMap>) -> MQTTResult<Option<Packet>> {
        self.publish(topic, true, message.as_bytes(), props)
    }

    /// Basic send of a UTF8 encoded payload. The message is sent with QoS
    /// Level 0 and a payload format indicated a UTF8. No additional properties
    /// are set. A disconnect message can occur due to errors. This method uses
    /// a simple blocking read with a timeout for test purposes.
    pub fn publish(&mut self, topic: &str, utf8: bool, data: &[u8], props: Option<&UserPropertyMap>) -> MQTTResult<Option<Packet>> {
        let mut publish = Publish::default();
        publish.payload_utf8 = utf8;
        publish.topic_name = Some(topic.to_string());
        publish.set_payload(Vec::from(data));
        publish.user_props = props.cloned();

        self.send(Packet::Publish(publish))
    }
    
    pub fn send(&mut self, packet: Packet) -> MQTTResult<Option<Packet>> {
        let mut dest = BytesMut::default();
        let result = encode(packet, &mut dest);
        if let Err(e) = result {
            assert!(false, "Failed to encode packet: {:?}", e);
        }
        if let Err(e) = self.connection.as_ref().unwrap().write_all(&dest.to_vec()) {
            eprintln!("unexpected send error {:#?}", e);
            // TODO higher fidelity error handling
            return Err(MQTTError{kind: ErrorKind::Transport, message: e.to_string()});
        }
        Ok(None)
    }

    pub fn subscribe(&mut self, packet_id: u16, topic_filter: &str) -> MQTTResult<()> {
        let mut subscribe = Subscribe::default();
        subscribe.set_packet_id(packet_id);
        let subscription = Subscription {
            filter: topic_filter.to_string(),
            ..Default::default()
        };
        subscribe.add_subscription(subscription);
        let mut dest = BytesMut::default();
        let result = encode(Packet::Subscribe(subscribe.clone()), &mut dest);
        if let Err(e) = result {
            assert!(false, "Failed to encode packet: {:?}", e);
        }
        match self.connection.as_ref().unwrap().write_all(&dest.to_vec()) {
            Ok(_) => Ok(()),
            Err(e) => Err(MQTTError {
                message: format!("unexpected send error {:#?}", e),
                kind: ErrorKind::Transport,
            }),
        }
    }

    pub fn read_next(&mut self) -> MQTTResult<Option<Packet>> {
        let mut buffer = [0u8; 4096];
        match self.connection.as_ref().unwrap().read(&mut buffer) {
            Ok(len) => match decode(&mut BytesMut::from(&buffer[0..len])) {
                Ok(packet) => {
                    println!("**** packet in read");
                    Ok(packet)
                }
                Err(e) => 
                Err(MQTTError {
                    message: e.reason,
                    kind: ErrorKind::Codec,
                }),
            },
            Err(e) => match e.kind() {
                std::io::ErrorKind::TimedOut => Err(MQTTError {
                    message: e.to_string(),
                    kind: ErrorKind::Timeout,
                }),
                _ => Err(MQTTError {
                    message: e.to_string(),
                    kind: ErrorKind::Transport,
                }),
            },
        }
    }

    pub fn set_read_timeout(&mut self, millis: u64) -> MQTTResult<()> {
        if self
            .connection
            .as_mut()
            .unwrap()
            .set_read_timeout(Some(Duration::from_millis(millis)))
            .is_err()
        {
            return Err(MQTTError::new(
                "unable to set connection read timeout",
                ErrorKind::Transport,
            ));
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
