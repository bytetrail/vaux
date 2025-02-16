use crate::MqttClient;
use crate::{stream::AsyncMqttStream, ErrorKind, MqttError};
use bytes::BytesMut;
use std::{collections::HashMap, sync::Arc, time::Duration, vec};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex, RwLock};
use vaux_mqtt::{
    decode, encode, property::Property, ConnAck, Connect, Packet, PropertyType, PubResp, QoSLevel,
    Reason, SubscriptionFilter,
};

const MAX_QUEUE_LEN: usize = 100;

pub(crate) struct ClientSession {
    client_id: Arc<Mutex<Option<String>>>,
    connected: Arc<RwLock<bool>>,
    stream: AsyncMqttStream,
    last_active: std::time::Instant,

    session_expiry: u32,
    subscriptions: Vec<SubscriptionFilter>,
    pending_publish: Vec<Packet>,
    pending_recv_ack: HashMap<u16, Packet>,
    receive_max: usize,
    qos_1_remaining: usize,
    pending_qos1: Arc<Mutex<Vec<Packet>>>,

    read_buffer: Vec<u8>,
    read_offset: usize,
    max_packet_size: usize,

    auto_packet_id: bool,
    last_packet_id: u16,
    auto_ack: bool,
}

impl ClientSession {
    pub(crate) async fn connected(&self) -> bool {
        *self.connected.read().await
    }

    pub(crate) async fn connect<'a>(
        &mut self,
        max_connect_wait: Duration,
        credentials: Option<(String, String)>,
        clean_start: bool,
    ) -> crate::Result<ConnAck> {
        let mut connect = Connect::default();
        connect.clean_start = clean_start;
        {
            let set_id = self.client_id.lock().await;
            if set_id.is_some() {
                connect.client_id = (*set_id.as_ref().unwrap()).to_string();
            }
        }
        connect
            .properties_mut()
            .set_property(Property::SessionExpiryInterval(self.session_expiry));
        if let Some((username, password)) = credentials {
            connect.username = Some(username);
            connect.password = Some(password.into_bytes());
        }
        let connect_packet = Packet::Connect(Box::new(connect));
        // let mut buffer = [0u8; DEFAULT_CHANNEL_SIZE];
        let mut dest = BytesMut::default();
        let result = encode(&connect_packet, &mut dest);
        if let Err(e) = result {
            panic!("Failed to encode packet: {:?}", e);
        }

        match self.stream.write_all(&dest).await {
            Ok(_) => {
                let start = std::time::Instant::now();
                while start.elapsed() < max_connect_wait {
                    match self.read_next().await {
                        Ok(Some(packet)) => match packet {
                            Packet::ConnAck(connack) => {
                                return self.handle_connack(connack).await;
                            }
                            Packet::Disconnect(disconnect) => {
                                return Err(MqttError::new(
                                    &format!("disconnect received: {}", disconnect.reason),
                                    ErrorKind::Protocol(disconnect.reason),
                                ))
                            }
                            _ => {
                                return Err(MqttError::new(
                                    "unexpected packet type",
                                    ErrorKind::Protocol(Reason::ProtocolErr),
                                ))
                            }
                        },
                        Ok(None) => {
                            return Err(MqttError::new(
                                "no MQTT packet received",
                                ErrorKind::Protocol(Reason::ProtocolErr),
                            ))
                        }
                        Err(e) => match e.kind {
                            ErrorKind::Timeout => {
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                            _ => {
                                return Err(MqttError::new(
                                    &format!("unable to read packet: {}", e),
                                    ErrorKind::Transport,
                                ));
                            }
                        },
                    }
                }
                Err(MqttError::new(
                    "unable to connect to broker",
                    ErrorKind::Timeout,
                ))
            }
            Err(e) => Err(MqttError::new(
                &format!("Unable to write packet(s) to broker: {}", e),
                ErrorKind::Transport,
            )),
        }
    }

    /// Reads the next packet from the stream, returning None if the stream is closed.
    /// This method is cancel safe. No data is lost if the task is cancelled at the
    /// await point.
    ///
    pub(crate) async fn read_next(&mut self) -> crate::Result<Option<Packet>> {
        let mut bytes_read = self.read_offset;
        loop {
            if bytes_read > 0 {
                let bytes_mut = &mut BytesMut::from(&self.read_buffer[0..bytes_read]);
                match decode(bytes_mut) {
                    Ok(data_read) => {
                        if let Some((packet, decode_len)) = data_read {
                            if decode_len < bytes_read as u32 {
                                self.read_buffer
                                    .copy_within(decode_len as usize..bytes_read, 0);
                                // adjust offset to end of decoded bytes
                                self.read_offset = bytes_read - decode_len as usize;
                            } else {
                                self.read_offset = 0;
                            }
                            return Ok(Some(packet));
                        } else {
                            return Ok(None);
                        }
                    }
                    Err(e) => match e.kind {
                        vaux_mqtt::codec::ErrorKind::InsufficientData(_expected, _actual) => {
                            // fall through the the socket read
                        }
                        _ => {
                            return Err(MqttError::new(
                                &e.to_string(),
                                crate::ErrorKind::Protocol(Reason::ProtocolErr),
                            ));
                        }
                    },
                }
            }
            match self
                .stream
                .read(&mut self.read_buffer[self.read_offset..self.max_packet_size])
                .await
            {
                Ok(len) => {
                    if len == 0 && bytes_read == 0 {
                        return Ok(None);
                    }
                    bytes_read += len;
                    self.read_offset = bytes_read;
                }
                Err(e) => match e.kind() {
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => {
                        return Err(MqttError::new(&e.to_string(), ErrorKind::Timeout));
                    }
                    e => {
                        return Err(MqttError::new(&e.to_string(), ErrorKind::IO));
                    }
                },
            }
        }
    }

    pub(crate) async fn write_next(&mut self, packet: Packet) -> Result<(), MqttError> {
        match packet {
            Packet::Publish(mut p) => {
                if p.qos() == QoSLevel::AtLeastOnce {
                    if self.auto_packet_id && p.packet_id.is_none() {
                        self.last_packet_id += 1;
                        p.packet_id = Some(self.last_packet_id);
                        self.pending_recv_ack
                            .insert(self.last_packet_id, Packet::Publish(p.clone()));
                    } else if let Some(packet_id) = p.packet_id {
                        self.pending_recv_ack
                            .insert(packet_id, Packet::Publish(p.clone()));
                    } else {
                        return Err(MqttError::new(
                            "no packet ID for QoS > 0",
                            ErrorKind::Protocol(Reason::MalformedPacket),
                        ));
                    }
                    // if we have additional capacity for QOS 1 PUBACK
                    if self.qos_1_remaining > 0 {
                        self.qos_1_remaining -= 1;
                    } else {
                        if self.pending_publish.len() < MAX_QUEUE_LEN {
                            self.pending_publish.push(Packet::Publish(p));
                            return Ok(());
                        } else {
                            let err = MqttError::new(
                                "unable to send packet, queue full",
                                ErrorKind::Protocol(Reason::ProtocolErr),
                            );
                            return Err(err);
                        }
                    }
                }
                self.send_packet(Packet::Publish(p)).await
            }
            Packet::Disconnect(d) => {
                self.send_packet(Packet::Disconnect(d)).await?;
                // TODO handle shutdown error?
                let _ = self.stream.shutdown().await;
                self.pending_qos1
                    .lock()
                    .await
                    .append(&mut self.pending_publish);
                let mut connected = self.connected.write().await;
                *connected = false;
                return Ok(());
            }
            _ => self.send_packet(packet).await,
        }
    }

    pub(crate) async fn handle_packet(
        &mut self,
        packet: Packet,
    ) -> Result<Option<Packet>, MqttError> {
        let mut packet_to_consumer = true;
        match &packet {
            Packet::PingResponse(_pingresp) => {
                // do not send to consumer
                packet_to_consumer = false;
            }
            Packet::Disconnect(d) => {
                // TODO handle disconnect - verify shutdown behavior
                let _ = self.stream.shutdown().await;
                self.pending_qos1
                    .lock()
                    .await
                    .append(&mut self.pending_publish);
                return Err(MqttError::new(
                    &format!("disconnect received: {:?}", d),
                    ErrorKind::Protocol(d.reason),
                ));
            }
            Packet::Publish(publish) => {
                match publish.qos() {
                    vaux_mqtt::QoSLevel::AtMostOnce => {}
                    vaux_mqtt::QoSLevel::AtLeastOnce => {
                        if self.auto_ack {
                            let mut puback = PubResp::new_puback();
                            if let Some(packet_id) = publish.packet_id {
                                puback.packet_id = packet_id;
                            } else {
                                let _ = self.stream.shutdown().await;
                                return Err(MqttError::new(
                                    "protocol error, packet ID required with QoS > 0",
                                    ErrorKind::Protocol(Reason::MalformedPacket),
                                ));
                            }
                            if self.send_packet(Packet::PubAck(puback)).await.is_err() {
                                // TODO handle the pub ack next time through
                                // push a message to the last error channel\
                                todo!()
                            }
                        }
                    }
                    vaux_mqtt::QoSLevel::ExactlyOnce => todo!(),
                }
            }
            Packet::PubAck(puback) => {
                if let Some(_p) = self.pending_recv_ack.remove(&puback.packet_id) {
                    if self.qos_1_remaining < self.receive_max {
                        self.qos_1_remaining += 1;
                    }
                } else {
                    // TODO PUBACK that was not expected
                }
            }
            _ => {}
        }
        if packet_to_consumer {
            return Ok(Some(packet));
        } else {
            return Ok(None);
        }
    }

    async fn handle_connack(&mut self, connack: ConnAck) -> crate::Result<ConnAck> {
        let set_id = self.client_id.lock().await;
        let client_id_set = set_id.is_some();
        if connack.reason() != Reason::Success {
            // TODO return the connack reason as MQTT error with reason code
            let mut connected = self.connected.write().await;
            *connected = false;
            return Err(MqttError::new(
                "connection refused",
                ErrorKind::Protocol(connack.reason()),
            ));
        } else {
            let mut connected = self.connected.write().await;
            *connected = true;
        }
        if !client_id_set {
            match connack
                .properties()
                .get_property(PropertyType::AssignedClientId)
            {
                Some(Property::AssignedClientId(id)) => {
                    let mut client_id = self.client_id.lock().await;
                    *client_id = Some(id.to_owned());
                }
                _ => {
                    // handle error here for required property
                    Err(MqttError::new(
                        "no assigned client id",
                        ErrorKind::Protocol(Reason::InvalidClientId),
                    ))?;
                }
            }
        }
        // TODO set server properties based on ConnAck
        Ok(connack)
    }

    pub(crate) async fn keep_alive(&mut self) -> Result<(), MqttError> {
        if !self.pending_publish.is_empty() && self.qos_1_remaining > 0 {
            // send any pending QOS-1 publish packets that we are able to send
            while !self.pending_publish.is_empty() && self.qos_1_remaining > 0 {
                while !self.pending_publish.is_empty() && self.qos_1_remaining > 0 {
                    let packet = self.pending_publish.remove(0);
                    if let Err(e) = self.send_packet(packet.clone()).await {
                        self.pending_publish.insert(0, packet);
                        return Err(e);
                    } else {
                        self.qos_1_remaining += 1;
                    }
                }
            }
            // packet sent, update last active time
            self.last_active = std::time::Instant::now();
        } else {
            let ping = Packet::PingRequest(Default::default());
            if let Err(e) = self.send_packet(ping).await {
                return Err(e);
            }
            // packet sent, update last active time
            self.last_active = std::time::Instant::now();
        }
        Ok(())
    }

    async fn send_packet(&mut self, packet: Packet) -> Result<(), MqttError> {
        let mut dest = BytesMut::default();
        let result = encode(&packet, &mut dest);
        if let Err(e) = result {
            panic!("Failed to encode packet: {:?}", e);
        }
        if let Err(e) = self.stream.write_all(&dest).await {
            return Err(MqttError::new(
                &format!("Unable to write packet(s) to broker: {}", e),
                ErrorKind::Transport,
            ));
        }
        Ok(())
    }
}

impl TryFrom<&mut MqttClient> for ClientSession {
    type Error = MqttError;

    fn try_from(client: &mut MqttClient) -> Result<Self, Self::Error> {
        if client.packet_in.is_none() {
            MqttError::new(
                "packet_in channel is required",
                ErrorKind::Protocol(Reason::ProtocolErr),
            );
        }
        if client.packet_out.is_none() {
            MqttError::new(
                "packet_out channel is required",
                ErrorKind::Protocol(Reason::ProtocolErr),
            );
        }
        if client.connection.is_none() {
            MqttError::new(
                "connection is required",
                ErrorKind::Protocol(Reason::ProtocolErr),
            );
        }

        Ok(Self {
            client_id: Arc::clone(&client.client_id),
            connected: Arc::new(RwLock::new(false)),
            stream: client.connection.take().unwrap().take_stream().unwrap(),
            last_active: std::time::Instant::now(),
            session_expiry: client.session_expiry(),
            subscriptions: vec![],
            pending_publish: vec![],
            pending_recv_ack: HashMap::new(),
            receive_max: client.receive_max as usize,
            qos_1_remaining: client.receive_max as usize,
            pending_qos1: Arc::new(Mutex::new(vec![])),
            read_buffer: vec![0u8; client.max_packet_size],
            read_offset: 0,
            max_packet_size: client.max_packet_size,
            auto_packet_id: client.auto_packet_id,
            last_packet_id: 0,
            auto_ack: client.auto_ack,
        })
    }
}
