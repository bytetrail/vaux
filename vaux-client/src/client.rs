use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    sync::{
        mpsc::{self, Receiver, SendError, Sender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use bytes::BytesMut;
use vaux_mqtt::{
    decode, encode, property::Property, Connect, Packet, PropertyType, PubResp, QoSLevel,
    Subscribe, Subscription,
};

use crate::{ErrorKind, MqttError};

const DEFAULT_CONNECT_INTERVAL: u64 = 3000;
const DEFAULT_CONNECT_RETRY: u8 = 20;
const DEFAULT_RECV_MAX: u16 = 100;
const DEFAULT_HOST_IP: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 1883;

const MAX_QUEUE_LEN: usize = 100;
// TODO add size tracking to pending publish
// const MAX_QUEUE_SIZE: usize = 100 * 1024;

pub type Result<T> = core::result::Result<T, MqttError>;

#[derive(Debug)]
pub struct MqttClient {
    auto_ack: bool,
    auto_packet_id: bool,
    last_packet_id: u16,
    receive_max: u16,
    addr: SocketAddr,
    connected: bool,
    connection: Option<TcpStream>,
    connect_retry: u8,
    connect_interval: u64,
    client_id: Option<String>,
    producer: Sender<vaux_mqtt::Packet>,
    consumer: Option<Receiver<vaux_mqtt::Packet>>,
    packet_send: Option<Receiver<vaux_mqtt::Packet>>,
    packet_recv: Option<Sender<vaux_mqtt::Packet>>,
    subscriptions: Vec<Subscription>,
    pending_qos1: Arc<Mutex<Vec<Packet>>>,
}

impl Default for MqttClient {
    fn default() -> Self {
        let ip_addr = DEFAULT_HOST_IP.parse::<Ipv4Addr>().unwrap();
        Self::new(
            IpAddr::V4(ip_addr),
            DEFAULT_PORT,
            &uuid::Uuid::new_v4().to_string(),
            true,
            DEFAULT_RECV_MAX,
            true,
        )
    }
}

impl MqttClient {
    pub fn new(
        host: IpAddr,
        port: u16,
        client_id: &str,
        auto_ack: bool,
        receive_max: u16,
        auto_packet_id: bool,
    ) -> Self {
        let (producer, packet_send): (Sender<vaux_mqtt::Packet>, Receiver<vaux_mqtt::Packet>) =
            mpsc::channel();
        let (packet_recv, consumer): (Sender<vaux_mqtt::Packet>, Receiver<vaux_mqtt::Packet>) =
            mpsc::channel();
        Self {
            auto_ack,
            auto_packet_id,
            last_packet_id: 0,
            receive_max,
            addr: SocketAddr::new(host, port),
            connected: false,
            connection: None,
            connect_retry: DEFAULT_CONNECT_RETRY,
            connect_interval: DEFAULT_CONNECT_INTERVAL,
            client_id: Some(client_id.to_string()),
            producer,
            consumer: Some(consumer),
            packet_send: Some(packet_send),
            packet_recv: Some(packet_recv),
            subscriptions: Vec::new(),
            pending_qos1: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn producer(&self) -> Sender<vaux_mqtt::Packet> {
        self.producer.clone()
    }

    pub fn take_consumer(&mut self) -> Option<Receiver<vaux_mqtt::Packet>> {
        self.consumer.take()
    }

    pub fn connect(&mut self) -> Result<()> {
        let mut result: Result<()> = Ok(());
        let mut retry = true;
        let mut attempts = 0;
        let interval = Duration::from_millis(self.connect_interval);
        while retry {
            result = self.connect_with_timeout(interval);
            if let Err(e) = &result {
                match e.kind() {
                    ErrorKind::Timeout => {}
                    _ => thread::sleep(interval),
                }
                attempts += 1;
                if attempts == self.connect_retry {
                    break;
                }
            } else {
                retry = false;
            }
        }
        result
    }

    /// Helper method to subscribe to the topics in the topic filter. This helper
    /// subscribes with a QoS level of "At Most Once", or 0. A SUBACK will
    /// typically be returned on the consumer on a successful subscribe.
    pub fn subscribe(
        &mut self,
        packet_id: u16,
        topic_filter: &[&str],
        qos: QoSLevel,
    ) -> std::result::Result<(), SendError<Packet>> {
        let mut subscribe = Subscribe::default();
        subscribe.set_packet_id(packet_id);
        for topic in topic_filter {
            let subscription = Subscription {
                filter: (*topic).to_string(),
                qos,
                ..Default::default()
            };
            self.subscriptions.push(subscription.clone());
            subscribe.add_subscription(subscription);
        }
        self.producer.send(vaux_mqtt::Packet::Subscribe(subscribe))
    }

    /// Starts the MQTT client thread. The MQTT client thread will send packets
    /// to the remote broker that it receives on the producer channel and make
    /// packets available on the consumer channel that it receives from the broker
    ///
    /// The MQTT client thread can be stopped by calling the stop method or by
    /// sending a DISCONNECT packet on the producer channel.
    ///
    /// There are cases where the client may not be able to send a message (e.g.
    /// QoS 1 and no more messages can be sent). In these cases, the message will
    /// be queued and sent when the client is able to send it or until the maximum
    /// queue size is reached based on packet size and/or count. The client will
    /// thread will terminate if the queue is full and the client is unable to send.
    ///
    /// Queued messages will be sent in the order they were received. Any messages
    /// that are queued when the client is stopped will remain queued until the client
    /// is started again or the client is dropped.
    pub fn start(&mut self) -> Option<JoinHandle<Result<()>>> {
        let packet_recv = self.packet_recv.take().unwrap();
        let packet_send = self.packet_send.take().unwrap();
        let auto_ack = self.auto_ack;
        let mut connection = self.connection.take().unwrap();
        let receive_max = self.receive_max;
        let pending_qos1 = self.pending_qos1.clone();
        let mut last_packet_id = self.last_packet_id;
        let auto_packet_id = self.auto_packet_id;
        Some(thread::spawn(move || {
            if let Err(e) = connection.set_read_timeout(Some(Duration::from_millis(100))) {
                return Err(MqttError::new(
                    &format!("unable to set read timeout: {}", e),
                    ErrorKind::Transport,
                ));
            }
            let mut pending_recv_ack: HashMap<u16, Packet> = HashMap::new();
            let mut pending_publish: Vec<Packet> = Vec::new();
            // TODO add size tracking to pending publish
            // let mut pending_publish_size = 0;
            let mut qos_1_remaining = receive_max;
            pending_publish.append(&mut pending_qos1.lock().unwrap());
            loop {
                match MqttClient::read_next(&mut connection) {
                    Ok(result) => {
                        if let Some(p) = result {
                            match &p {
                                Packet::Disconnect(d) => {
                                    // TODO handle disconnect - verify shutdown behavior
                                    connection.shutdown(std::net::Shutdown::Both).unwrap();
                                    pending_qos1.lock().unwrap().append(&mut pending_publish);
                                    return Err(MqttError::new(
                                        &format!("disconnect received: {:?}", d),
                                        ErrorKind::Protocol,
                                    ));
                                }
                                Packet::Publish(publish) => {
                                    match publish.qos() {
                                        vaux_mqtt::QoSLevel::AtMostOnce => {}
                                        vaux_mqtt::QoSLevel::AtLeastOnce => {
                                            if auto_ack {
                                                let mut puback = PubResp::new_puback();
                                                if let Some(packet_id) = publish.packet_id {
                                                    puback.packet_id = packet_id;
                                                } else {
                                                    connection
                                                        .shutdown(std::net::Shutdown::Both)
                                                        .unwrap();
                                                    return Err(MqttError::new(
                                                        "protocol error, no packet ID with QAS > 0",
                                                        ErrorKind::Protocol,
                                                    ));
                                                }
                                                if MqttClient::send(
                                                    &mut connection,
                                                    Packet::PubAck(puback),
                                                )
                                                .is_err()
                                                {
                                                    // TODO handle the pub ack next time through
                                                    // push a message to the last error channel
                                                    eprintln!("unable to send puback");
                                                }
                                            }
                                        }
                                        vaux_mqtt::QoSLevel::ExactlyOnce => todo!(),
                                    }
                                }
                                Packet::PubAck(puback) => {
                                    if let Some(_p) = pending_recv_ack.remove(&puback.packet_id) {
                                        if qos_1_remaining < receive_max {
                                            qos_1_remaining += 1;
                                        }
                                    } else {
                                        // TODO PUBACK that was not expected
                                    }
                                }
                                _ => {}
                            }
                            if let Err(e) = packet_recv.send(p.clone()) {
                                connection.shutdown(std::net::Shutdown::Both).unwrap();
                                pending_qos1.lock().unwrap().append(&mut pending_publish);
                                return Err(MqttError::new(
                                    &format!("unable to send packet to consumer: {}", e),
                                    ErrorKind::Transport,
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        if e.kind() != ErrorKind::Timeout {
                            // there may be nothing to read so this is not necessarily an error
                            // TODO configure for disconnect/reconnect, PING or stop on timeouts
                        }
                    }
                };
                if let Ok(packet) = packet_send.recv_timeout(Duration::from_millis(10)) {
                    if let Packet::Publish(mut p) = packet.clone() {
                        if p.qos() == QoSLevel::AtLeastOnce {
                            if qos_1_remaining > 0 {
                                qos_1_remaining -= 1;
                                if auto_packet_id {
                                    last_packet_id += 1;
                                    p.packet_id = Some(last_packet_id);
                                    pending_recv_ack.insert(last_packet_id, packet.clone());
                                } else if let Some(packet_id) = p.packet_id {
                                    pending_recv_ack.insert(packet_id, packet.clone());
                                } else {
                                }
                            } else {
                                // TODO cannot send the packet - need to inform client
                                if pending_publish.len() < MAX_QUEUE_LEN {
                                    // && pending_publish_size < MAX_QUEUE_SIZE {
                                    pending_publish.push(packet);
                                    continue;
                                }
                            }
                        }
                    } else if let Packet::Disconnect(_d) = packet.clone() {
                        if let Err(e) = MqttClient::send(&mut connection, packet) {
                            eprintln!("ERROR sending packet to remote: {}", e.message());
                        }
                        connection.shutdown(std::net::Shutdown::Both).unwrap();
                        pending_qos1.lock().unwrap().append(&mut pending_publish);
                        return Ok(());
                    }
                    if let Err(e) = MqttClient::send(&mut connection, packet) {
                        eprintln!("ERROR sending packet to remote: {}", e.message());
                    }
                    // send any pending QOS-1 publish packets that we are able to send
                    while pending_publish.len() > 0 && qos_1_remaining > 0 {
                        let packet = pending_publish.remove(0);
                        // pending_publish_size -= packet.encoded_size();
                        if let Err(e) = MqttClient::send(&mut connection, packet.clone()) {
                            pending_publish.insert(0, packet);
                            // TODO notify calling client of error
                            eprintln!("ERROR sending packet to remote: {}", e.message());
                        } else {
                            qos_1_remaining += 1;
                        }
                    }
                }
            }
        }))
    }

    pub fn stop(&mut self) {
        let disconnect = Packet::Disconnect(Default::default());
        if let Err(e) = self.producer.send(disconnect) {
            eprintln!("unable to send disconnect: {}", e);
        }
    }

    fn connect_with_timeout(&mut self, timeout: Duration) -> Result<()> {
        let mut connect = Connect::default();
        if let Some(id) = self.client_id.as_ref() {
            connect.client_id = id.to_owned();
        }
        let connect_packet = Packet::Connect(Box::new(connect));
        match TcpStream::connect_timeout(&self.addr, timeout) {
            Ok(stream) => {
                self.connection = Some(stream);
                let mut buffer = [0u8; 128];
                let mut dest = BytesMut::default();
                let result = encode(connect_packet, &mut dest);
                if let Err(e) = result {
                    panic!("Failed to encode packet: {:?}", e);
                }
                match self.connection.as_ref().unwrap().write_all(&dest) {
                    Ok(_) => match self.connection.as_ref().unwrap().read(&mut buffer) {
                        Ok(len) => match decode(&mut BytesMut::from(&buffer[0..len])) {
                            Ok(p) => {
                                if let Some(packet) = p {
                                    match packet {
                                        Packet::ConnAck(connack) => {
                                            if self.client_id.is_none() {
                                                match connack
                                                    .properties()
                                                    .get_property(&PropertyType::AssignedClientId)
                                                {
                                                    Some(Property::AssignedClientId(id)) => {
                                                        self.client_id = Some(id.to_owned());
                                                    }
                                                    _ => {
                                                        // handle error here for required property
                                                        Err(MqttError::new(
                                                            "no assigned client id",
                                                            ErrorKind::Protocol,
                                                        ))?;
                                                    }
                                                }
                                            }
                                            // TODO set server properties based on ConnAck
                                            self.connected = true;
                                            Ok(())
                                        }
                                        Packet::Disconnect(_disconnect) => {
                                            // TODO return the disconnect reason as MQTT error
                                            panic!("disconnect");
                                        }
                                        _ => Err(MqttError::new(
                                            "unexpected packet type",
                                            ErrorKind::Protocol,
                                        )),
                                    }
                                } else {
                                    Err(MqttError::new(
                                        "no MQTT packet received",
                                        ErrorKind::Protocol,
                                    ))
                                }
                            }
                            Err(e) => Err(MqttError::new(&e.to_string(), ErrorKind::Codec)),
                        },
                        Err(e) => Err(MqttError::new(
                            &format!("unable to read stream: {}", e),
                            ErrorKind::Transport,
                        )),
                    },
                    Err(e) => panic!("Unable to write packet(s) to test broker: {}", e),
                }
            }
            Err(e) => match e.kind() {
                std::io::ErrorKind::TimedOut => Err(MqttError {
                    message: "timeout".to_string(),
                    kind: ErrorKind::Timeout,
                }),
                _ => Err(MqttError::new(
                    &format!("unable to connect: {}", e),
                    ErrorKind::Connection,
                )),
            },
        }
    }

    pub fn read_next(connection: &mut TcpStream) -> Result<Option<Packet>> {
        let mut buffer = [0u8; 4096];
        match connection.read(&mut buffer) {
            Ok(len) => match decode(&mut BytesMut::from(&buffer[0..len])) {
                Ok(packet) => Ok(packet),
                Err(e) => Err(MqttError {
                    message: e.reason,
                    kind: ErrorKind::Codec,
                }),
            },
            Err(e) => match e.kind() {
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => Err(MqttError {
                    message: e.to_string(),
                    kind: ErrorKind::Timeout,
                }),
                _ => Err(MqttError {
                    message: e.to_string(),
                    kind: ErrorKind::Transport,
                }),
            },
        }
    }

    pub fn send(connection: &mut TcpStream, packet: Packet) -> Result<Option<Packet>> {
        let mut dest = BytesMut::default();
        let result = encode(packet, &mut dest);
        if let Err(e) = result {
            panic!("Failed to encode packet: {:?}", e);
        }
        if let Err(e) = connection.write_all(&dest) {
            eprintln!("unexpected send error {:#?}", e);
            // TODO higher fidelity error handling
            return Err(MqttError {
                kind: ErrorKind::Transport,
                message: e.to_string(),
            });
        }
        Ok(None)
    }
}
