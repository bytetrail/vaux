use crate::{ErrorKind, MqttClient, MqttError};
use std::{collections::HashMap, sync::Arc, time::Duration, vec};
use tokio::sync::{Mutex, RwLock};
use vaux_mqtt::WillMessage;
use vaux_mqtt::{
    property::Property, ConnAck, Connect, Packet, PropertyType, PubResp, QoSLevel, Reason,
};
const MAX_QUEUE_LEN: usize = 100;

#[allow(dead_code)]
pub struct SessionState {
    client_id: Arc<Mutex<Option<String>>>,
    pending_publish: Vec<Packet>,
    pending_recv_ack: HashMap<u16, Packet>,

    qos_1_remaining: usize,
    pending_qos1: Arc<Mutex<Vec<Packet>>>,
}

pub(crate) struct ClientSession {
    client_id: Arc<Mutex<Option<String>>>,
    connected: Arc<RwLock<bool>>,
    packet_stream: vaux_async::stream::PacketStream,
    last_active: std::time::Instant,

    session_expiry: u32,
    pending_publish: Vec<Packet>,
    pending_recv_ack: HashMap<u16, Packet>,
    receive_max: usize,
    qos_1_remaining: usize,
    pending_qos1: Arc<Mutex<Vec<Packet>>>,

    auto_packet_id: bool,
    last_packet_id: u16,
    auto_ack: bool,
    pingresp: bool,
}

impl ClientSession {
    pub(crate) async fn connected(&self) -> bool {
        *self.connected.read().await
    }

    pub(crate) async fn connect(
        &mut self,
        max_connect_wait: Duration,
        keep_alive: Duration,
        credentials: Option<(String, String)>,
        clean_start: bool,
        will: Option<WillMessage>,
    ) -> crate::Result<ConnAck> {
        let mut connect = Connect::default();
        connect.keep_alive = keep_alive.as_secs() as u16;
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
        if let Some(will) = will {
            connect.will_message = Some(will);
        }
        let connect_packet = Packet::Connect(Box::new(connect));

        match self.packet_stream.write(&connect_packet).await {
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
        self.packet_stream.read().await.map_err(|e| {
            MqttError::new(
                &format!("unable to read packet: {}", e),
                ErrorKind::Transport,
            )
        })
    }

    pub(crate) async fn write_next(&mut self, packet: Packet) -> Result<(), MqttError> {
        match packet {
            Packet::Publish(mut p) => {
                if p.qos() == QoSLevel::AtLeastOnce {
                    if self.auto_packet_id && p.packet_id.is_none() {
                        self.last_packet_id = self.last_packet_id.wrapping_add(1);
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
                    } else if self.pending_publish.len() < MAX_QUEUE_LEN {
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
                self.send_packet(Packet::Publish(p)).await
            }
            Packet::Disconnect(d) => {
                self.send_packet(Packet::Disconnect(d)).await?;
                // TODO handle shutdown error?
                let _ = self.packet_stream.shutdown().await;
                self.pending_qos1
                    .lock()
                    .await
                    .append(&mut self.pending_publish);
                *self.connected.write().await = false;
                Ok(())
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
                packet_to_consumer = !self.pingresp;
            }
            Packet::Disconnect(d) => {
                // TODO handle disconnect - verify shutdown behavior
                let _ = self.packet_stream.shutdown().await;
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
                                let _ = self.packet_stream.shutdown().await;
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
            Ok(Some(packet))
        } else {
            Ok(None)
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
            self.send_packet(ping).await?;
            // packet sent, update last active time
            self.last_active = std::time::Instant::now();
        }
        Ok(())
    }

    async fn send_packet(&mut self, packet: Packet) -> Result<(), MqttError> {
        self.packet_stream
            .write(&packet)
            .await
            .map_err(|_| MqttError::new("unable to write packet to stream", ErrorKind::Transport))
    }
}

impl TryFrom<&mut MqttClient> for ClientSession {
    type Error = MqttError;

    fn try_from(client: &mut MqttClient) -> Result<Self, Self::Error> {
        if client.connection.is_none() {
            MqttError::new(
                "connection is required",
                ErrorKind::Protocol(Reason::ProtocolErr),
            );
        }

        Ok(Self {
            client_id: Arc::clone(&client.client_id),
            connected: Arc::new(RwLock::new(false)),
            packet_stream: vaux_async::stream::PacketStream::new(
                client.connection.take().unwrap().take_stream().unwrap(),
                None,
                Some(client.max_packet_size),
            ),
            last_active: std::time::Instant::now(),
            session_expiry: client.session_expiry,
            pending_publish: vec![],
            pending_recv_ack: HashMap::new(),
            receive_max: client.receive_max as usize,
            qos_1_remaining: client.receive_max as usize,
            pending_qos1: Arc::new(Mutex::new(vec![])),
            auto_packet_id: client.auto_packet_id,
            last_packet_id: 0,
            auto_ack: client.auto_ack,
            pingresp: client.pingresp,
        })
    }
}
