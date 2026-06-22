pub mod config;
pub(crate) mod session;
pub(crate) mod topic;

pub use config::Config;
use config::{BROKER_KEEP_ALIVE_FACTOR, DEFAULT_KEEP_ALIVE};
use session::Session;
use session::SessionControl;
use session::SessionPool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::{
    net::TcpListener,
    select,
    sync::{mpsc::Receiver, mpsc::Sender, RwLock},
    task::{yield_now, JoinHandle},
};
use topic::TopicTree;
use uuid::Uuid;
use vaux_async::stream::{AsyncMqttStream, MqttStream, PacketStream};
use vaux_mqtt::subscribe::SubAck;
use vaux_mqtt::unsubscribe::{UnsubAck, Unsubscribe};
use vaux_mqtt::Packet::PingResponse;
use vaux_mqtt::{
    ConnAck, Connect, Disconnect, MqttCodecError, Packet, PingResp, Publish, QoSLevel, Reason,
    Subscribe, WillMessage,
};

const INIT_STREAM_BUFFER_SIZE: usize = 4096;

#[derive(Debug)]
pub enum BrokerError {
    Codec(MqttCodecError),
    Io(std::io::Error),
    Stream(vaux_async::stream::Error),
}

impl std::fmt::Display for BrokerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            BrokerError::Codec(e) => write!(f, "codec error: {e}"),
            BrokerError::Io(e) => write!(f, "io error: {e}"),
            BrokerError::Stream(e) => write!(f, "stream error {e}"),
        }
    }
}

impl From<std::io::Error> for BrokerError {
    fn from(e: std::io::Error) -> Self {
        BrokerError::Io(e)
    }
}

impl From<MqttCodecError> for BrokerError {
    fn from(e: MqttCodecError) -> Self {
        BrokerError::Codec(e)
    }
}

impl From<vaux_async::stream::Error> for BrokerError {
    fn from(e: vaux_async::stream::Error) -> Self {
        BrokerError::Stream(e)
    }
}

pub enum BrokerCommand {
    Stop,
    PauseAccept,
    ResumeAccept,
    SetSessionExpiry(Duration),
}

#[derive(Debug, Clone)]
pub struct BrokerState {
    config: Arc<config::Config>,
    session_pool: Arc<RwLock<SessionPool>>,
    topic_tree: Arc<RwLock<TopicTree>>,
    accept_connections: Arc<AtomicBool>,
}

impl BrokerState {
    fn new(config: config::Config) -> Self {
        let session_pool = SessionPool::new_with_config(&config);
        Self {
            config: Arc::new(config),
            session_pool: Arc::new(RwLock::new(session_pool)),
            topic_tree: Arc::new(RwLock::new(TopicTree::new())),
            accept_connections: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn accepting_connections(&self) -> bool {
        self.accept_connections.load(Ordering::Acquire)
    }
}

#[derive(Debug)]
pub struct Broker {
    state: BrokerState,
    command_channel: (Sender<BrokerCommand>, Option<Receiver<BrokerCommand>>),
}

impl Default for Broker {
    fn default() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        Broker {
            state: BrokerState::new(config::Config::default()),
            command_channel: (sender, Some(receiver)),
        }
    }
}

impl Broker {
    pub fn new_with_config(config: config::Config) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        Broker {
            state: BrokerState::new(config),
            command_channel: (sender, Some(receiver)),
        }
    }

    pub async fn set_session_expiration(&mut self, expiration: Duration) {
        self.state
            .session_pool
            .write()
            .await
            .set_session_expiry(expiration);
    }

    pub async fn stop(&mut self) {}

    pub async fn run(&mut self) -> Result<(), BrokerError> {
        tracing::info!(addr = %self.state.config.listen_addr, "broker starting");
        let state = self.state.clone();
        match TcpListener::bind(self.state.config.listen_addr).await {
            Ok(listener) => {
                let mut command = self.command_channel.1.take().unwrap();

                let _handle: JoinHandle<Result<(), BrokerError>> = tokio::spawn(async move {
                    loop {
                        select! {
                            cmd = command.recv() => {
                                match cmd {
                                    Some(BrokerCommand::Stop) => {
                                        return Ok(());
                                    }
                                    Some(BrokerCommand::PauseAccept) => {
                                        state.accept_connections.store(false, Ordering::Release);
                                    }
                                    Some(BrokerCommand::ResumeAccept) => {
                                        state.accept_connections.store(true, Ordering::Release);
                                    }
                                    Some(BrokerCommand::SetSessionExpiry(expiration)) => {
                                        state.session_pool.write().await.set_session_expiry(expiration);
                                    }
                                    None => {
                                        return Ok(());
                                    }
                                }
                            }
                            result = listener.accept() => {
                                match result {
                                    Ok((socket, _)) => {
                                        let client_state = state.clone();
                                        let tls = client_state.config.tls_acceptor.clone();
                                        tokio::spawn(async move {
                                            let mqtt_stream = if let Some(acceptor) = tls {
                                                match acceptor.accept(socket).await {
                                                    Ok(tls_stream) => {
                                                        MqttStream::ServerTlsStream(Box::new(tls_stream))
                                                    }
                                                    Err(e) => {
                                                        tracing::warn!(error = %e, "TLS handshake failed");
                                                        return Ok(());
                                                    }
                                                }
                                            } else {
                                                MqttStream::TcpStream(socket)
                                            };
                                            let mut stream = PacketStream::new(
                                                AsyncMqttStream(mqtt_stream),
                                                Some(INIT_STREAM_BUFFER_SIZE),
                                                None,
                                            );
                                            Broker::handle_client(&mut stream, client_state).await
                                        });
                                    }
                                    Err(e) => {
                                        tracing::error!(error = %e, "error accepting connection");
                                        return Err(e.into());
                                    }
                                }
                            }
                        }
                    }
                });
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn handle_client(
        stream: &mut PacketStream,
        state: BrokerState,
    ) -> Result<(), BrokerError> {
        let mut connected = false;
        let mut client_session: Option<Arc<RwLock<Session>>> = None;
        let mut session_control: Option<Receiver<SessionControl>> = None;
        let mut keep_alive = DEFAULT_KEEP_ALIVE;
        loop {
            select! {
                ctrl = Broker::handle_control(&mut session_control) => {
                    if let Some(control) = ctrl {
                        match control {
                            SessionControl::Disconnect(reason) => {
                                if let Some(session) = &client_session {
                                    let session_id = {
                                        let session = session.read().await;
                                        (*session).id().to_string()
                                    };
                                    match reason {
                                        Reason::SessionTakeOver => {
                                            // the session will already be deactivated and taken over
                                            // by another client, just send the disconnect packet
                                        }
                                        _ => {
                                            state.session_pool.write().await.deactivate(&session_id).await;
                                        }
                                    }
                                    stream.write(&mut Packet::Disconnect(Disconnect::new(reason))).await?;
                                } else {
                                    stream.shutdown().await?;
                                    return Ok(());
                                }
                            }
                            SessionControl::Deliver(publish) => {
                                stream.write(&mut Packet::Publish(*publish)).await?;
                            }
                        }
                    }
                }
                _ = Broker::keep_alive_timer(keep_alive) => {
                    if let Some(session) = &client_session {
                        let session_id = session.read().await.id().to_string();
                        tracing::info!(session_id = %session_id, "keep-alive expired");
                        Broker::initiate_will(session, &state).await;
                        state.session_pool.write().await.deactivate(&session_id).await;
                    } else {
                        stream.shutdown().await?;
                        return Ok(());
                    }
                }
                packet_result = stream.read() => {
                    match packet_result {
                        Ok(Some(packet)) => {
                            if !connected {
                                if let Some(session) = Broker::handle_pre_connect(packet, stream, &state).await? {
                                    let pending = {
                                        let mut session = session.write().await;
                                        keep_alive = session.keep_alive();
                                        session_control = session.take_control_receiver();
                                        session.drain_pending()
                                    };
                                    for (packet_id, mut publish) in pending {
                                        publish.set_dup(true).ok();
                                        let _ = publish.set_packet_id(Some(packet_id));
                                        {
                                            let mut s = session.write().await;
                                            s.track_qos_publish(packet_id, publish.clone());
                                        }
                                        stream.write(&mut Packet::Publish(publish)).await?;
                                    }
                                    client_session = Some(Arc::clone(&session));
                                } else {
                                    continue;
                                }
                                connected = true;
                            } else if let Some(session) = &client_session {
                                Broker::handle_packet(packet, stream, Arc::clone(session), &state).await?;
                            }
                        }
                        Ok(None) => {
                            if let Some(session) = &client_session {
                                let session_id = session.read().await.id().to_string();
                                tracing::info!(session_id = %session_id, "connection closed");
                                Broker::initiate_will(session, &state).await;
                                state.session_pool.write().await.deactivate(&session_id).await;
                            }
                            return Ok(());
                        }
                        Err(e) => {
                            if let Some(session) = &client_session {
                                let session_id = session.read().await.id().to_string();
                                tracing::warn!(session_id = %session_id, error = %e, "connection error");
                                Broker::initiate_will(session, &state).await;
                                state.session_pool.write().await.deactivate(&session_id).await;
                            }
                            return Err(e.into());
                        }
                    }
                }
            }
        }
    }

    async fn handle_packet(
        packet: Packet,
        stream: &mut PacketStream,
        session: Arc<RwLock<Session>>,
        state: &BrokerState,
    ) -> Result<(), BrokerError> {
        match packet {
            Packet::PingRequest(_p) => {
                let mut packet = PingResponse(PingResp::default());
                stream.write(&mut packet).await?;
                Ok(())
            }
            Packet::Subscribe(subscribe) => {
                let session_id = {
                    let session = session.read().await;
                    session.id().to_string()
                };
                let suback = Broker::handle_subscribe(&subscribe, &session_id, state).await;
                stream.write(&mut Packet::SubAck(suback)).await?;
                Ok(())
            }
            Packet::Unsubscribe(unsubscribe) => {
                let session_id = {
                    let session = session.read().await;
                    session.id().to_string()
                };
                let unsuback = Broker::handle_unsubscribe(&unsubscribe, &session_id, state).await;
                stream.write(&mut Packet::UnsubAck(unsuback)).await?;
                Ok(())
            }
            Packet::Publish(publish) => {
                let session_id = {
                    let session = session.read().await;
                    session.id().to_string()
                };
                if publish.qos() == QoSLevel::AtLeastOnce {
                    if let Some(packet_id) = publish.packet_id() {
                        let puback = vaux_mqtt::PubAck::new_puback_with_packet_id(packet_id);
                        stream.write(&mut Packet::PubAck(puback)).await?;
                    }
                }
                Broker::route_publish(&publish, &session_id, state).await;
                Ok(())
            }
            Packet::PubAck(puback) => {
                let mut session = session.write().await;
                session.acknowledge(puback.packet_id);
                Ok(())
            }
            Packet::Disconnect(packet) => {
                if let Reason::Success = packet.reason {
                    let mut session = session.write().await;
                    session.clear_will();
                } else {
                    Broker::initiate_will(&session, state).await;
                }
                let session_id = {
                    let session = session.read().await;
                    (*session).id().to_string()
                };
                state
                    .session_pool
                    .write()
                    .await
                    .deactivate(&session_id)
                    .await;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    async fn handle_subscribe(
        subscribe: &Subscribe,
        session_id: &str,
        state: &BrokerState,
    ) -> SubAck {
        let mut suback = SubAck::new_with_packet_id(subscribe.packet_id).unwrap_or_default();
        let mut tree = state.topic_tree.write().await;
        for filter in &subscribe.filter {
            let granted_qos = filter.qos();
            tree.subscribe(&filter.filter, session_id, granted_qos, filter.no_local());
            let reason = match granted_qos {
                vaux_mqtt::QoSLevel::AtMostOnce => Reason::Success,
                vaux_mqtt::QoSLevel::AtLeastOnce => Reason::GrantedQoS1,
                vaux_mqtt::QoSLevel::ExactlyOnce => Reason::GrantedQoS2,
            };
            suback.reason_codes.push(reason);
        }
        suback
    }

    async fn handle_unsubscribe(
        unsubscribe: &Unsubscribe,
        session_id: &str,
        state: &BrokerState,
    ) -> UnsubAck {
        let mut unsuback = UnsubAck::default();
        unsuback.packet_id = unsubscribe.packet_id;
        let mut tree = state.topic_tree.write().await;
        for topic in &unsubscribe.topics {
            let reason = if tree.unsubscribe(topic, session_id) {
                Reason::Success
            } else {
                Reason::NoSubscribers
            };
            unsuback.reason_code.push(reason);
        }
        unsuback
    }

    async fn handle_control(
        control_channel: &mut Option<Receiver<SessionControl>>,
    ) -> Option<SessionControl> {
        loop {
            if let Some(rx) = control_channel {
                if let Some(control) = rx.recv().await {
                    return Some(control);
                }
            }
            yield_now().await;
        }
    }

    async fn handle_pre_connect(
        packet: Packet,
        stream: &mut PacketStream,
        state: &BrokerState,
    ) -> Result<Option<Arc<RwLock<Session>>>, BrokerError> {
        match packet {
            Packet::Connect(packet) => {
                if !state.accepting_connections() {
                    tracing::info!("connection rejected: not accepting connections");
                    let mut ack = ConnAck::default();
                    ack.reason = Reason::ServerBusy;
                    stream.write(&mut Packet::ConnAck(ack)).await?;
                    stream.shutdown().await?;
                    return Ok(None);
                }
                Broker::handle_connect(*packet, stream, state).await
            }
            Packet::PingRequest(_packet) => {
                let mut packet = PingResponse(PingResp::default());
                stream.write(&mut packet).await?;
                Ok(None)
            }
            _ => {
                let header = Disconnect::new(Reason::ProtocolErr);
                let mut packet = Packet::Disconnect(header);
                stream.write(&mut packet).await?;
                Ok(None)
            }
        }
    }

    async fn handle_connect(
        connect: Connect,
        stream: &mut PacketStream,
        state: &BrokerState,
    ) -> Result<Option<Arc<RwLock<Session>>>, BrokerError> {
        let session_id: String;
        let mut ack = ConnAck::default();
        let (default_keep_alive_secs, max_keep_alive_secs) = {
            let session_pool = state.session_pool.read().await;
            (
                session_pool.default_keep_alive_secs(),
                session_pool.max_keep_alive_secs(),
            )
        };
        // handle keep alive request
        let keep_alive = if connect.keep_alive < default_keep_alive_secs {
            ack.set_server_keep_alive(default_keep_alive_secs);
            Duration::from_secs((default_keep_alive_secs as f32 * BROKER_KEEP_ALIVE_FACTOR) as u64)
        } else if connect.keep_alive > max_keep_alive_secs {
            ack.set_server_keep_alive(max_keep_alive_secs);
            Duration::from_secs((max_keep_alive_secs as f32 * BROKER_KEEP_ALIVE_FACTOR) as u64)
        } else {
            Duration::from_secs((connect.keep_alive as f32 * BROKER_KEEP_ALIVE_FACTOR) as u64)
        };
        // handle the session expiry request
        let session_expiry = if let Some(requested_expiry) = connect.session_expiry_interval {
            if requested_expiry > 0 {
                let max_expiry: u32;
                {
                    max_expiry = state.session_pool.read().await.session_expiry().as_secs() as u32;
                }
                if requested_expiry > max_expiry {
                    ack.set_session_expiry(max_expiry);
                    Some(Duration::from_secs(max_expiry as u64))
                } else {
                    Some(Duration::from_secs(requested_expiry as u64))
                }
            } else {
                None
            }
        } else {
            None
        };
        // handle the client id
        if connect.client_id.is_empty() {
            session_id = Uuid::new_v4().to_string();
            ack.assigned_client_id = Some(session_id.clone());
        } else {
            session_id = connect.client_id.to_string();
        }
        let mut session_pool = state.session_pool.write().await;
        let max_active = state.config.max_active_sessions;
        let channel_size = state.config.session_channel_size;

        let has_active_session = session_pool.get_active(&session_id).is_some();
        if !has_active_session && session_pool.at_capacity(max_active) {
            drop(session_pool);
            tracing::info!(session_id = %session_id, "connection rejected: max active sessions reached");
            ack.reason = Reason::QuotaExceeded;
            stream.write(&mut Packet::ConnAck(ack)).await?;
            stream.shutdown().await?;
            return Ok(None);
        }

        let session: Arc<RwLock<Session>> = if connect.clean_start() {
            if let Some(existing_session) = session_pool.remove(session_id.as_str()) {
                let connected = { existing_session.read().await.connected() };
                if connected {
                    let _ = Broker::handle_takeover(Arc::clone(&existing_session)).await;
                }
            }
            let mut new_session =
                Session::new(session_id.clone(), connect.will_message(), keep_alive, channel_size);
            new_session.set_connected(true);
            let new_session = Arc::new(RwLock::new(new_session));
            session_pool.add(Arc::clone(&new_session)).await;
            new_session
        } else {
            if let Some(session) = session_pool.activate(&session_id).await {
                {
                    let mut session = session.write().await;
                    ack.set_session_present(true);
                    session.cancel_pending_will();
                    session.set_will(connect.will_message());
                    session.reset_control(channel_size);
                }
                session
            } else if let Some(session) = session_pool.get_active(&session_id) {
                {
                    let _ = Broker::handle_takeover(Arc::clone(&session)).await;
                    let mut session = session.write().await;
                    ack.set_session_present(true);
                    session.cancel_pending_will();
                    session.set_will(connect.will_message());
                    session.reset_control(channel_size);
                    session.set_last_active();
                }
                session
            } else {
                let mut new_session = Session::new(
                    session_id.clone(),
                    connect.will_message().clone(),
                    keep_alive,
                    channel_size,
                );
                new_session.set_connected(true);
                let new_session = Arc::new(RwLock::new(new_session));
                session_pool.add(Arc::clone(&new_session)).await;
                new_session
            }
        };
        drop(session_pool);
        {
            let mut session = session.write().await;
            if let Some(expiry) = session_expiry {
                session.set_session_expiry(expiry);
            }
        }
        tracing::info!(session_id = %session_id, "client connected");
        stream.write(&mut Packet::ConnAck(ack)).await?;
        Ok(Some(Arc::clone(&session)))
    }

    async fn keep_alive_timer(keep_alive: Duration) {
        tokio::time::sleep(keep_alive).await;
    }

    async fn handle_takeover(session: Arc<RwLock<Session>>) -> Result<(), BrokerError> {
        session
            .read()
            .await
            .disconnect(Reason::SessionTakeOver)
            .await;
        // TODO wait for disconnect to complete from session state
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    async fn initiate_will(session: &Arc<RwLock<Session>>, state: &BrokerState) {
        let will = {
            let mut session = session.write().await;
            session.take_will()
        };

        if let Some(will) = will {
            tracing::debug!(topic = %will.topic, "publishing will message");
            let delay_secs = will.header.will_delay.unwrap_or(0);
            if delay_secs == 0 {
                Broker::publish_will(&will, state).await;
            } else {
                let token = tokio_util::sync::CancellationToken::new();
                let child_token = token.child_token();
                let state = state.clone();
                tokio::spawn(async move {
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(delay_secs as u64)) => {
                            Broker::publish_will(&will, &state).await;
                        }
                        _ = child_token.cancelled() => {}
                    }
                });
                let mut session = session.write().await;
                session.set_pending_will(token);
            }
        }
    }

    async fn publish_will(will: &WillMessage, state: &BrokerState) {
        let mut publish = Publish::new_from_header(vaux_mqtt::FixedHeader::new(
            vaux_mqtt::PacketType::Publish,
        ))
        .unwrap();
        publish.topic_name = will.topic.clone();
        publish.set_qos(will.qos);
        publish.set_retain(will.retain);
        publish.payload = Some(will.payload.clone());
        publish.message_expiry = will.header.message_expiry;
        publish.content_type = will.header.content_type.clone();
        publish.response_topic = will.header.response_topic.clone();
        publish.correlation_data = will.header.correlation_data.clone();
        if let Some(format) = will.header.payload_format {
            publish.set_payload_format(format);
        }
        Broker::route_publish(&publish, "", state).await;
    }

    async fn route_publish(publish: &Publish, sender_session_id: &str, state: &BrokerState) {
        let subscribers = {
            let tree = state.topic_tree.read().await;
            tree.matching_subscribers(&publish.topic_name)
        };
        let session_pool = state.session_pool.read().await;
        for sub in &subscribers {
            if sub.no_local && *sub.session_id == *sender_session_id {
                continue;
            }
            let granted_qos = std::cmp::min(publish.qos() as u8, sub.qos as u8);
            let qos: QoSLevel = granted_qos.try_into().unwrap_or(QoSLevel::AtMostOnce);

            if let Some(target_session) = session_pool.get_active(&sub.session_id) {
                let mut out = publish.clone();
                out.set_qos(qos);
                if qos != QoSLevel::AtMostOnce {
                    let mut target = target_session.write().await;
                    let packet_id = target.next_packet_id();
                    let _ = out.set_packet_id(Some(packet_id));
                    target.track_qos_publish(packet_id, out.clone());
                    if let Err(_) = target.try_send_control(SessionControl::Deliver(Box::new(out))) {
                        tracing::warn!(
                            session_id = %sub.session_id,
                            topic = %publish.topic_name,
                            "slow subscriber: channel full, disconnecting"
                        );
                        let _ = target.try_send_control(SessionControl::Disconnect(Reason::QuotaExceeded));
                    }
                } else {
                    let target = target_session.read().await;
                    if let Err(_) = target.try_send_control(SessionControl::Deliver(Box::new(out))) {
                        tracing::warn!(
                            session_id = %sub.session_id,
                            topic = %publish.topic_name,
                            "slow subscriber: channel full, disconnecting"
                        );
                        let _ = target.try_send_control(SessionControl::Disconnect(Reason::QuotaExceeded));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::{
        net::{Ipv4Addr, SocketAddr},
        str::FromStr,
    };

    use crate::config::{
        DEFAULT_LISTEN_ADDR, DEFAULT_PORT, DEFAULT_SESSION_EXPIRY, MAX_KEEP_ALIVE,
    };

    use super::*;

    #[test]
    fn test_default() {
        const EXPECTED_IP_ADDR: &str = "127.0.0.1";
        const EXPECTED_PORT: u16 = 1883;
        let broker = Broker::default();
        assert!(
            broker.state.config.listen_addr.is_ipv4(),
            "expected IPV4 address"
        );
        assert_eq!(
            EXPECTED_IP_ADDR,
            broker.state.config.listen_addr.ip().to_string(),
            "expected local loopback address: 127.0.0.1"
        );
        assert_eq!(
            EXPECTED_PORT,
            broker.state.config.listen_addr.port(),
            "expected default listen port to be 1883"
        );
    }

    #[test]
    fn test_new() {
        const EXPECTED_IP_ADDR: &str = "127.0.0.1";
        const EXPECTED_PORT: u16 = 1883;

        let listen_addr = SocketAddr::try_from((
            Ipv4Addr::from_str(DEFAULT_LISTEN_ADDR).unwrap(),
            DEFAULT_PORT,
        ))
        .unwrap();

        let broker = Broker::new_with_config(config::Config {
            listen_addr,
            default_keep_alive: DEFAULT_KEEP_ALIVE,
            max_keep_alive: MAX_KEEP_ALIVE,
            session_expiry: DEFAULT_SESSION_EXPIRY,
            ..Default::default()
        });
        assert_eq!(
            EXPECTED_IP_ADDR,
            broker.state.config.listen_addr.ip().to_string(),
            "expected local loopback address: 127.0.0.1"
        );
        assert_eq!(
            EXPECTED_PORT,
            broker.state.config.listen_addr.port(),
            "expected default listen port to be 1883"
        );
    }
}
