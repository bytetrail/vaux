use crate::{ErrorKind, MqttClient, MqttError};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{Mutex, RwLock};
use vaux_mqtt::publish::Publish;
use vaux_mqtt::WillMessage;
use vaux_mqtt::{property::Property, ConnAck, Connect, Packet, PubResp, QoSLevel, Reason};

const DEFAULT_MAX_PACKET_SIZE: usize = 64 * 1024;
const DEFAULT_CLIENT_ID_PREFIX: &str = "vaux-client";
const DEFAULT_CLIENT_KEEP_ALIVE: Duration = Duration::from_secs(60);
const DEFAULT_SESSION_EXPIRY: Duration = Duration::from_secs(600);

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
enum QoSPacket {
    PubAck,
    PubRec,
    PubRel,
    PubComp,
}

#[derive(Debug)]
struct QoSPacketState {
    packet_id: u16,
    #[allow(dead_code)]
    packet: Option<Packet>,
    next: QoSPacket,
}

impl QoSPacketState {
    pub fn new_qos1(packet: Packet) -> Self {
        Self {
            packet_id: 0,
            packet: Some(packet),
            next: QoSPacket::PubAck,
        }
    }

    pub fn new_qos2(packet: Packet) -> Self {
        Self {
            packet_id: 0,
            packet: Some(packet),
            next: QoSPacket::PubRec,
        }
    }

    pub fn next(self) -> Option<Self> {
        match self.next {
            QoSPacket::PubAck => None,
            QoSPacket::PubRec => Some(Self {
                packet_id: self.packet_id,
                packet: None,
                next: QoSPacket::PubRel,
            }),
            QoSPacket::PubRel => Some(Self {
                packet_id: self.packet_id,
                packet: None,
                next: QoSPacket::PubComp,
            }),
            QoSPacket::PubComp => None,
        }
    }
}

#[derive(Debug)]
pub struct SessionState {
    pub(crate) client_id: Arc<Mutex<Option<String>>>,
    pub(crate) keep_alive: Arc<RwLock<Duration>>,
    pub(crate) session_expiry: Arc<RwLock<Duration>>,

    pub(crate) qos_send_remaining: usize,
    pending_qos_send: HashMap<u16, QoSPacketState>,
    pub(crate) qos_recv_remaining: usize,
    pending_qos_recv: HashMap<u16, QoSPacketState>,

    pub(crate) max_packet_size: usize,
    pub(crate) auto_packet_id: bool,
    pub(crate) last_packet_id: u16,
    pub(crate) auto_ack: bool,
    pub(crate) pingresp: bool,
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            client_id: Arc::new(Mutex::new(Some(format!(
                "{}-{}",
                DEFAULT_CLIENT_ID_PREFIX,
                uuid::Uuid::new_v4()
            )))),
            keep_alive: Arc::new(RwLock::new(DEFAULT_CLIENT_KEEP_ALIVE)),
            session_expiry: Arc::new(RwLock::new(DEFAULT_SESSION_EXPIRY)),
            qos_send_remaining: u16::MAX as usize,
            pending_qos_send: HashMap::new(),
            qos_recv_remaining: u16::MAX as usize,
            pending_qos_recv: HashMap::new(),

            max_packet_size: DEFAULT_MAX_PACKET_SIZE,
            auto_packet_id: true,
            last_packet_id: 0,
            auto_ack: true,
            pingresp: false,
        }
    }
}

pub(crate) struct ClientSession {
    connected: Arc<RwLock<bool>>,
    packet_stream: vaux_async::stream::PacketStream,
    last_active: std::time::Instant,
    state: SessionState,
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
            connected: Arc::new(RwLock::new(false)),
            packet_stream: vaux_async::stream::PacketStream::new(
                client.connection.take().unwrap().take_stream().unwrap(),
                None,
                Some(
                    client
                        .session_state
                        .as_ref()
                        .map_or(DEFAULT_MAX_PACKET_SIZE, |s| s.max_packet_size),
                ),
            ),
            last_active: std::time::Instant::now(),
            state: client
                .session_state
                .take()
                .ok_or_else(SessionState::new)
                .unwrap(),
        })
    }

    pub(crate) async fn connected(&self) -> bool {
        *self.connected.read().await
    }

    pub(crate) async fn keep_alive(&self) -> Duration {
        *self.state.keep_alive.read().await
    }

    pub(crate) async fn session_expiry(&self) -> Duration {
        *self.state.session_expiry.read().await
    }

    pub(crate) fn auto_ack(&self) -> bool {
        self.state.auto_ack
    }

    pub(crate) async fn connect(
        &mut self,
        max_connect_wait: Duration,
        credentials: Option<(String, String)>,
        clean_start: bool,
        will: Option<WillMessage>,
    ) -> crate::Result<ConnAck> {
        let mut connect = Connect::default();
        connect.keep_alive = self.state.keep_alive.read().await.as_secs() as u16;
        connect.clean_start = clean_start;
        {
            let set_id = self.state.client_id.lock().await;
            if set_id.is_some() {
                connect.client_id = (*set_id.as_ref().unwrap()).to_string();
            }
        }
        connect
            .properties_mut()
            .set_property(Property::SessionExpiryInterval(
                self.state.session_expiry.read().await.as_secs() as u32,
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
                    if self.state.pending_qos_send.len() >= u16::MAX as usize
                        || self.state.qos_send_remaining == 0
                    {
                        return Err(MqttError::new(
                            "pending QoS packets full",
                            ErrorKind::Protocol(Reason::ProtocolErr),
                        ));
                    }
                    // verify client set packet ID is not already in use
                    if let Some(packet_id) = p.packet_id {
                        if self.state.pending_qos_send.contains_key(&packet_id) {
                            return Err(MqttError::new(
                                "packet ID already in use",
                                ErrorKind::Protocol(Reason::MalformedPacket),
                            ));
                        }
                    } else if self.state.auto_packet_id {
                        // if auto packet ID is enabled, generate a packet ID
                        self.state.last_packet_id = self.state.last_packet_id.wrapping_add(1);
                        // verify the packet ID is not already in use
                        // TODO improve in-use packet ID managment
                        while self
                            .state
                            .pending_qos_send
                            .contains_key(&self.state.last_packet_id)
                        {
                            self.state.last_packet_id = self.state.last_packet_id.wrapping_add(1);
                        }
                        p.packet_id = Some(self.state.last_packet_id);
                    } else {
                        // if no packet ID is set and auto packet ID is disabled, return an error
                        return Err(MqttError::new(
                            "no packet ID for QoS > 0",
                            ErrorKind::Protocol(Reason::MalformedPacket),
                        ));
                    }
                    match p.qos() {
                        QoSLevel::AtLeastOnce => {
                            self.state.pending_qos_send.insert(
                                p.packet_id.unwrap(),
                                QoSPacketState::new_qos1(Packet::Publish(p.clone())),
                            );
                        }
                        QoSLevel::ExactlyOnce => {
                            self.state.pending_qos_send.insert(
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
                packet_to_consumer = !self.state.pingresp;
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
                self.handle_recv_publish(publish.clone()).await?;
            }
            Packet::PubAck(puback) => {
                if let Some(p) = self.state.pending_qos_send.get(&puback.packet_id) {
                    match p.next {
                        QoSPacket::PubAck => {
                            self.state.pending_qos_send.remove(&puback.packet_id);
                            self.state.qos_send_remaining += 1;
                        }
                        _ => {
                            // protocol error, unexpected packet - disconnect
                            return Err(MqttError::new(
                                "unexpected control packet",
                                ErrorKind::Protocol(Reason::ProtocolErr),
                            ));
                        }
                    }
                } else {
                    // protocol error, unexpected packet - disconnect
                    return Err(MqttError::new(
                        "unexpected packet",
                        ErrorKind::Protocol(Reason::ProtocolErr),
                    ));
                }
            }
            Packet::PubRec(pubrec) => {
                let packet_id = pubrec.packet_id;
                if let Some(p) = self.state.pending_qos_send.remove(&packet_id) {
                    match p.next {
                        QoSPacket::PubRec => {
                            if self.state.auto_ack {
                                // insert PUBREL into pending qos
                                let pubrec_state = p.next().ok_or(MqttError::new(
                                    "invalid session state",
                                    ErrorKind::Session,
                                ))?;
                                self.state.pending_qos_send.insert(packet_id, pubrec_state);
                                // create the PUBREL packet
                                let mut pubrel = PubResp::new_pubrel();
                                pubrel.packet_id = pubrec.packet_id;
                                match self.send_packet(Packet::PubRel(pubrel)).await {
                                    Ok(_) => {
                                        // remove from pending qos
                                        if let Some(pubrec_packet) =
                                            self.state.pending_qos_send.remove(&packet_id)
                                        {
                                            match pubrec_packet.next {
                                                QoSPacket::PubRel => {
                                                    self.state.pending_qos_send.insert(
                                                        packet_id,
                                                        pubrec_packet.next().ok_or(
                                                            MqttError::new(
                                                                "invalid session state",
                                                                ErrorKind::Session,
                                                            ),
                                                        )?,
                                                    );
                                                }
                                                _ => {
                                                    // protocol error, unexpected packet
                                                    // re-insert into pending qos without change
                                                    self.state
                                                        .pending_qos_send
                                                        .insert(packet_id, pubrec_packet);
                                                    return Err(MqttError::new(
                                                        "unexpected control packet",
                                                        ErrorKind::Protocol(Reason::ProtocolErr),
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        return Err(MqttError::new(
                                            "unable to send pub rel",
                                            ErrorKind::Transport,
                                        ));
                                    }
                                }
                            }
                        }
                        _ => {
                            // protocol error, unexpected packet
                            // re-insert into pending qos without change
                            self.state.pending_qos_send.insert(packet_id, p);
                            // TODO disconnect
                            return Err(MqttError::new(
                                "unexpected control packet",
                                ErrorKind::Protocol(Reason::ProtocolErr),
                            ));
                        }
                    }
                } else {
                    // protocol error, unexpected packet
                    // TODO disconnect
                    return Err(MqttError::new(
                        "unexpected packet",
                        ErrorKind::Protocol(Reason::ProtocolErr),
                    ));
                }
            }
            Packet::PubRel(pubrel) => {
                let packet_id = pubrel.packet_id;
                if let Some(p) = self.state.pending_qos_recv.remove(&packet_id) {
                    match p.next {
                        QoSPacket::PubRel => {
                            if self.state.auto_ack {
                                let mut pubcomp = PubResp::new_pubcomp();
                                pubcomp.packet_id = pubrel.packet_id;
                                match self.send_packet(Packet::PubComp(pubcomp)).await {
                                    Ok(_) => {
                                        // remove from pending qos
                                        self.state.pending_qos_recv.remove(&packet_id);
                                        self.state.qos_recv_remaining += 1;
                                    }
                                    Err(_) => {
                                        // re-insert into pending qos without change
                                        self.state.pending_qos_recv.insert(packet_id, p);
                                        return Err(MqttError::new(
                                            "unable to send pub comp",
                                            ErrorKind::Transport,
                                        ));
                                    }
                                }
                            }
                        }
                        _ => {
                            // protocol error, unexpected packet
                            // re-insert into pending qos without change
                            self.state.pending_qos_recv.insert(packet_id, p);
                            return Err(MqttError::new(
                                "unexpected packet",
                                ErrorKind::Protocol(Reason::ProtocolErr),
                            ));
                        }
                    }
                } else {
                    // protocol error, unexpected packet
                    // TODO disconnect
                    return Err(MqttError::new(
                        "unexpected packet",
                        ErrorKind::Protocol(Reason::ProtocolErr),
                    ));
                }
            }
            Packet::PubComp(pubcomp) => {
                let packet_id = pubcomp.packet_id;
                if let Some(p) = self.state.pending_qos_send.remove(&packet_id) {
                    match p.next {
                        QoSPacket::PubComp => {
                            //  do nothing - removed from pending qos
                            self.state.qos_send_remaining += 1;
                        }
                        _ => {
                            // protocol error, unexpected packet
                            // re-insert into pending qos without change
                            self.state.pending_qos_send.insert(packet_id, p);
                            return Err(MqttError::new(
                                "unexpected control packet",
                                ErrorKind::Protocol(Reason::ProtocolErr),
                            ));
                        }
                    }
                } else {
                    // protocol error, unexpected packet
                    // TODO disconnect");
                    return Err(MqttError::new(
                        "unexpected packet",
                        ErrorKind::Protocol(Reason::ProtocolErr),
                    ));
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
        let set_id = self.state.client_id.lock().await;
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
                    let mut set_id = self.state.client_id.lock().await;
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
        if let Some(server_keep_alive) = connack.server_keep_alive() {
            let mut keep_alive = self.state.keep_alive.write().await;
            *keep_alive = Duration::from_secs(u64::from(server_keep_alive));
        }
        // set the server assigned session expiry if present
        if let Some(server_session_expiry) = connack.session_expiry() {
            let mut session_expiry = self.state.session_expiry.write().await;
            *session_expiry = Duration::from_secs(u64::from(server_session_expiry));
        }
        // set the server assigned receive maximum
        if let Some(receive_max) = connack.receive_max() {
            self.state.qos_send_remaining = receive_max as usize;
        }
        Ok(connack)
    }

    async fn handle_recv_publish(&mut self, publish: Publish) -> crate::Result<()> {
        match publish.qos() {
            vaux_mqtt::QoSLevel::AtMostOnce => Ok(()),

            vaux_mqtt::QoSLevel::AtLeastOnce => {
                if self.state.qos_recv_remaining == 0 {
                    // protocol error, no more QoS packets allowed
                    return Err(MqttError::new(
                        "no more QoS packets allowed",
                        ErrorKind::Protocol(Reason::ProtocolErr),
                    ));
                }
                // insert into pending receive QoS
                self.state.pending_qos_recv.insert(
                    publish.packet_id.unwrap(),
                    QoSPacketState::new_qos1(Packet::Publish(publish.clone())),
                );
                if self.state.auto_ack {
                    let mut puback = PubResp::new_puback();
                    if let Some(packet_id) = publish.packet_id {
                        puback.packet_id = packet_id;
                    } else {
                        // TODO send disconnect with reason cod
                        let _ = self.packet_stream.shutdown().await;
                        return Err(MqttError::new(
                            "protocol error, packet ID required with QoS > 0",
                            ErrorKind::Protocol(Reason::MalformedPacket),
                        ));
                    }
                    self.state.qos_recv_remaining -= 1;
                    match self.send_packet(Packet::PubAck(puback)).await {
                        Ok(_) => {
                            self.state
                                .pending_qos_recv
                                .remove(&publish.packet_id.unwrap());
                            self.state.qos_recv_remaining += 1;
                            Ok(())
                        }
                        Err(_) => {
                            // TODO handle the pub ack next time through
                            // push a message to the last error channel
                            Err(MqttError::new(
                                "unable to send pub ack",
                                ErrorKind::Transport,
                            ))
                        }
                    }
                } else {
                    // not auto ack, remaining - 1 until client app sends response
                    self.state.qos_recv_remaining -= 1;
                    Ok(())
                }
            }
            vaux_mqtt::QoSLevel::ExactlyOnce => {
                self.state.pending_qos_recv.insert(
                    publish.packet_id.unwrap(),
                    QoSPacketState::new_qos2(Packet::Publish(publish.clone())),
                );
                // decrement the remaining QoS packets until ack  sent
                self.state.qos_recv_remaining -= 1;
                if self.state.auto_ack {
                    let mut pubrec = PubResp::new_pubrec();
                    if let Some(packet_id) = publish.packet_id {
                        pubrec.packet_id = packet_id;
                    } else {
                        // protocol error, packet ID required with QoS > 0
                        let _ = self.packet_stream.shutdown().await;
                        return Err(MqttError::new(
                            "protocol error, packet ID required with QoS > 0",
                            ErrorKind::Protocol(Reason::MalformedPacket),
                        ));
                    }
                    match self.send_packet(Packet::PubRec(pubrec)).await {
                        Ok(_) => {
                            // move to next state and re-insert into pending qos
                            let next = self
                                .state
                                .pending_qos_recv
                                .remove(&publish.packet_id.unwrap())
                                .map(|p| p.next().unwrap())
                                .ok_or(MqttError::new(
                                    "invalid session state",
                                    ErrorKind::Session,
                                ))?;
                            self.state
                                .pending_qos_recv
                                .insert(publish.packet_id.unwrap(), next);
                        }
                        Err(_) => {
                            // TODO handle the pub rec next time through
                            // push a message to the last error channel
                            return Err(MqttError::new(
                                "unable to send pub rec",
                                ErrorKind::Transport,
                            ));
                        }
                    }
                }
                Ok(())
            }
        }
    }

    pub(crate) async fn handle_keep_alive(&mut self) -> Result<(), MqttError> {
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
