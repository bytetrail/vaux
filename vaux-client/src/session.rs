use crate::{ErrorKind, MqttClient, MqttError};
use std::{collections::HashMap, sync::Arc, time::Duration, vec};
use tokio::sync::{Mutex, RwLock};
use vaux_mqtt::WillMessage;
use vaux_mqtt::{property::Property, ConnAck, Connect, Packet, PubResp, QoSLevel, Reason};
const MAX_QUEUE_LEN: usize = 100;

enum QoSPacket {
    PubAck,
    PubRec,
    PubRel,
    PubComp,
}

struct QoSPacketState {
    packet: Packet,
    next: QoSPacket,
}

impl QoSPacketState {
    pub fn new_qos1(packet: Packet) -> Self {
        Self {
            packet,
            next: QoSPacket::PubAck,
        }
    }

    pub fn new_qos2(packet: Packet) -> Self {
        Self {
            packet,
            next: QoSPacket::PubRec,
        }
    }

    pub fn next(self) -> Option<Self> {
        match self.next {
            QoSPacket::PubAck => None,
            QoSPacket::PubRec => Some(Self {
                packet: self.packet,
                next: QoSPacket::PubRel,
            }),
            QoSPacket::PubRel => Some(Self {
                packet: self.packet,
                next: QoSPacket::PubComp,
            }),
            QoSPacket::PubComp => None,
        }
    }
}

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
    keep_alive: Duration,

    session_expiry: Duration,
    // pending_publish: Vec<Packet>,
    // pending_recv_ack: HashMap<u16, Packet>,
    receive_max: usize,
    qos_1_remaining: usize,
    pending_qos: HashMap<u16, QoSPacketState>,

    auto_packet_id: bool,
    last_packet_id: u16,
    auto_ack: bool,
    pingresp: bool,
}

impl ClientSession {
    pub(crate) async fn new_from_client(client: &mut MqttClient) -> Result<Self, MqttError> {
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
            session_expiry: *client.session_expiry.read().await,
            pending_qos: HashMap::new(),
            receive_max: client.receive_max as usize,
            qos_1_remaining: client.receive_max as usize,
            auto_packet_id: client.auto_packet_id,
            last_packet_id: 0,
            auto_ack: client.auto_ack,
            pingresp: client.pingresp,
            keep_alive: *client.keep_alive.read().await,
        })
    }

    pub(crate) async fn connected(&self) -> bool {
        *self.connected.read().await
    }

    pub(crate) fn keep_alive(&self) -> Duration {
        self.keep_alive
    }

    pub(crate) fn session_expiry(&self) -> Duration {
        self.session_expiry
    }

    pub(crate) async fn connect(
        &mut self,
        max_connect_wait: Duration,
        credentials: Option<(String, String)>,
        clean_start: bool,
        will: Option<WillMessage>,
    ) -> crate::Result<ConnAck> {
        let mut connect = Connect::default();
        connect.keep_alive = self.keep_alive.as_secs() as u16;
        connect.clean_start = clean_start;
        {
            let set_id = self.client_id.lock().await;
            if set_id.is_some() {
                connect.client_id = (*set_id.as_ref().unwrap()).to_string();
            }
        }
        connect
            .properties_mut()
            .set_property(Property::SessionExpiryInterval(
                self.session_expiry.as_secs() as u32,
            ));
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
                if p.qos() == QoSLevel::AtLeastOnce || p.qos() == QoSLevel::ExactlyOnce {
                    // check the pending qos for available space
                    if self.pending_qos.len() >= u16::max_value() as usize {
                        return Err(MqttError::new(
                            "pending QoS packets full",
                            ErrorKind::Protocol(Reason::ProtocolErr),
                        ));
                    }
                    // verify client set packet ID is not already in use
                    if let Some(packet_id) = p.packet_id {
                        if self.pending_qos.contains_key(&packet_id) {
                            return Err(MqttError::new(
                                "packet ID already in use",
                                ErrorKind::Protocol(Reason::MalformedPacket),
                            ));
                        }
                    } else if self.auto_packet_id {
                        // if auto packet ID is enabled, generate a packet ID
                        self.last_packet_id = self.last_packet_id.wrapping_add(1);
                        p.packet_id = Some(self.last_packet_id);
                    } else {
                        // if no packet ID is set and auto packet ID is disabled, return an error
                        return Err(MqttError::new(
                            "no packet ID for QoS > 0",
                            ErrorKind::Protocol(Reason::MalformedPacket),
                        ));
                    }
                    match p.qos() {
                        QoSLevel::AtLeastOnce => {
                            self.pending_qos.insert(
                                p.packet_id.unwrap(),
                                QoSPacketState::new_qos1(Packet::Publish(p.clone())),
                            );
                        }
                        QoSLevel::ExactlyOnce => {
                            self.pending_qos.insert(
                                p.packet_id.unwrap(),
                                QoSPacketState::new_qos2(Packet::Publish(p.clone())),
                            );
                        }
                        _ => {}
                    }
                }
                self.send_packet(Packet::Publish(p)).await
            }
            Packet::Disconnect(d) => {
                self.send_packet(Packet::Disconnect(d)).await?;
                // TODO handle shutdown error?
                let _ = self.packet_stream.shutdown().await;
                //self.pending_qos.append(&mut self.pending_publish);
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
                //self.pending_qos1.append(&mut self.pending_publish);
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
                    vaux_mqtt::QoSLevel::ExactlyOnce => {
                        if self.auto_ack {
                            let mut pubrec = PubResp::new_pubrec();
                            if let Some(packet_id) = publish.packet_id {
                                pubrec.packet_id = packet_id;
                            } else {
                                let _ = self.packet_stream.shutdown().await;
                                return Err(MqttError::new(
                                    "protocol error, packet ID required with QoS > 0",
                                    ErrorKind::Protocol(Reason::MalformedPacket),
                                ));
                            }
                            if self.send_packet(Packet::PubRec(pubrec)).await.is_err() {
                                // TODO handle the pub rec next time through
                                // push a message to the last error channel
                                todo!()
                            }
                        }
                    }
                }
            }
            Packet::PubAck(puback) => {
                if let Some(p) = self.pending_qos.get(&puback.packet_id) {
                    match p.next {
                        QoSPacket::PubAck => {
                            self.pending_qos.remove(&puback.packet_id);
                        }
                        _ => {
                            // protocol error, unexpected packet
                        }
                    }
                } else {
                    // protocol error, unexpected packet
                }
            }
            Packet::PubRec(pubrec) => {
                let packet_id = pubrec.packet_id;
                if let Some(p) = self.pending_qos.remove(&packet_id) {
                    match p.next {
                        QoSPacket::PubRec => {
                            // push to next state and re-insert into pending qos
                            let next = p.next().expect("pub rec next");
                            self.pending_qos.insert(packet_id, next);
                            if self.auto_ack {
                                let mut pubrel = PubResp::new_pubrel();
                                pubrel.packet_id = pubrec.packet_id;
                                if self.send_packet(Packet::PubRel(pubrel)).await.is_err() {
                                    // TODO handle the pub rel next time through
                                    // push a message to the last error channel
                                    todo!()
                                } else {
                                    // move to next state and re-insert into pending qos
                                    let next = self
                                        .pending_qos
                                        .remove(&packet_id)
                                        .expect("expected pubrel");
                                    let next = next.next().expect("pub rec next");
                                    self.pending_qos.insert(packet_id, next);
                                }
                            }
                        }
                        _ => {
                            // protocol error, unexpected packet
                            // re-insert into pending qos without change
                            self.pending_qos.insert(packet_id, p);
                        }
                    }
                } else {
                    // protocol error, unexpected packet
                }
            }
            Packet::PubRel(pubrel) => {
                let packet_id = pubrel.packet_id;
                if let Some(p) = self.pending_qos.remove(&packet_id) {
                    match p.next {
                        QoSPacket::PubRel => {
                            if self.auto_ack {
                                let mut pubcomp = PubResp::new_pubcomp();
                                pubcomp.packet_id = pubrel.packet_id;
                                if self.send_packet(Packet::PubComp(pubcomp)).await.is_err() {
                                    // TODO handle the pub comp next time through
                                    // push a message to the last error channel
                                    todo!()
                                }
                            }
                        }
                        _ => {
                            // protocol error, unexpected packet
                            // re-insert into pending qos without change
                            self.pending_qos.insert(packet_id, p);
                        }
                    }
                } else {
                    // protocol error, unexpected packet
                }
            }
            Packet::PubComp(pubcomp) => {
                let packet_id = pubcomp.packet_id;
                if let Some(p) = self.pending_qos.remove(&packet_id) {
                    match p.next {
                        QoSPacket::PubComp => {
                            //  do nothing - removed from pending qos
                        }
                        _ => {
                            // protocol error, unexpected packet
                            // re-insert into pending qos without change
                            self.pending_qos.insert(packet_id, p);
                        }
                    }
                } else {
                    // protocol error, unexpected packet
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
        // if the client ID was not set, set it from the assigned client ID
        if !client_id_set {
            match connack.assigned_client_id() {
                Some(client_id) => {
                    let mut set_id = self.client_id.lock().await;
                    *set_id = Some(client_id.clone());
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
        // set session keep alive from the server
        if let Some(keep_alive) = connack.server_keep_alive() {
            self.keep_alive = Duration::from_secs(u64::from(keep_alive));
        }
        // set the server assigned session expiry if present
        if let Some(session_expiry) = connack.session_expiry() {
            self.session_expiry = Duration::from_secs(u64::from(session_expiry));
        }
        Ok(connack)
    }

    pub(crate) async fn handle_keep_alive(&mut self) -> Result<(), MqttError> {
        // if !self.pending_publish.is_empty() && self.qos_1_remaining > 0 {
        //     // send any pending QOS-1 publish packets that we are able to send
        //     while !self.pending_publish.is_empty() && self.qos_1_remaining > 0 {
        //         while !self.pending_publish.is_empty() && self.qos_1_remaining > 0 {
        //             let packet = self.pending_publish.remove(0);
        //             if let Err(e) = self.send_packet(packet.clone()).await {
        //                 self.pending_publish.insert(0, packet);
        //                 return Err(e);
        //             } else {
        //                 self.qos_1_remaining += 1;
        //             }
        //         }
        //     }
        //     // packet sent, update last active time
        //     self.last_active = std::time::Instant::now();
        // } else {
        let ping = Packet::PingRequest(Default::default());
        self.send_packet(ping).await?;
        // packet sent, update last active time
        self.last_active = std::time::Instant::now();
        // }
        Ok(())
    }

    async fn send_packet(&mut self, packet: Packet) -> Result<(), MqttError> {
        self.packet_stream
            .write(&packet)
            .await
            .map_err(|_| MqttError::new("unable to write packet to stream", ErrorKind::Transport))
    }
}
