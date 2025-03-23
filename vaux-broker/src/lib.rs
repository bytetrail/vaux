pub(crate) mod codec;
pub mod config;
pub(crate) mod session;

pub use config::Config;
use config::{BROKER_KEEP_ALIVE_FACTOR, DEFAULT_KEEP_ALIVE};
use session::Session;
use session::SessionControl;
use session::SessionPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::{
    net::TcpListener,
    select,
    sync::{mpsc::Receiver, mpsc::Sender, RwLock},
    task::{yield_now, JoinHandle},
};
use uuid::Uuid;
use vaux_async::stream::{AsyncMqttStream, MqttStream, PacketStream};
use vaux_mqtt::Packet::PingResponse;
use vaux_mqtt::{
    ConnAck, Connect, Disconnect, FixedHeader, MqttCodecError, Packet, PacketType, Reason,
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
            BrokerError::Codec(e) => write!(f, "codec error: {}", e),
            BrokerError::Io(e) => write!(f, "io error: {}", e),
            BrokerError::Stream(e) => write!(f, "stream error {}", e),
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

#[derive(Debug)]
pub struct Broker {
    config: config::Config,
    session_pool: Arc<RwLock<SessionPool>>,
    command_channel: (Sender<BrokerCommand>, Option<Receiver<BrokerCommand>>),
}

impl Default for Broker {
    fn default() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        Broker {
            config: config::Config::default(),
            session_pool: Arc::new(RwLock::new(SessionPool::new_with_expiry(
                config::DEFAULT_SESSION_EXPIRY,
            ))),
            command_channel: (sender, Some(receiver)),
        }
    }
}

impl Broker {
    pub fn new_with_config(config: config::Config) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        Broker {
            config: config.clone(),
            session_pool: Arc::new(RwLock::new(SessionPool::new_with_config(&config))),
            command_channel: (sender, Some(receiver)),
        }
    }

    pub async fn set_session_expiration(&mut self, expiration: Duration) {
        self.session_pool
            .write()
            .await
            .set_session_expiry(expiration);
    }

    pub async fn stop(&mut self) {}

    pub async fn run(&mut self) -> Result<(), BrokerError> {
        // TODO recreate the command channel here
        println!("broker accepting request on {:?}", self.config.listen_addr);
        let session_pool = Arc::clone(&self.session_pool);
        match TcpListener::bind(self.config.listen_addr).await {
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
                                        // TODO pause accept
                                    }
                                    Some(BrokerCommand::ResumeAccept) => {
                                        // TODO resume accept
                                    }
                                    Some(BrokerCommand::SetSessionExpiry(expiration)) => {
                                        session_pool.write().await.set_session_expiry(expiration);
                                    }
                                    None => {
                                        return Ok(());
                                    }
                                }
                            }
                            result = listener.accept() => {
                                match result {
                                    Ok((socket, _)) => {
                                        // create an mqtt stream from the tcp stream
                                        let mut stream = PacketStream::new(
                                            AsyncMqttStream(MqttStream::TcpStream(socket)),
                                            Some(INIT_STREAM_BUFFER_SIZE),
                                            None,
                                        );
                                        let _session_pool = Arc::clone(&session_pool);
                                        tokio::spawn(async move {
                                            Broker::handle_client(&mut stream, _session_pool).await
                                        });
                                    }
                                    Err(e) => {
                                        println!("broker error accepting connection: {}", e);
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
        session_pool: Arc<RwLock<SessionPool>>,
    ) -> Result<(), BrokerError> {
        let mut connected = false;
        let mut client_session: Option<Arc<RwLock<Session>>> = None;
        let mut session_control: Option<Receiver<SessionControl>> = None;
        let mut keep_alive = DEFAULT_KEEP_ALIVE;
        loop {
            // TODO separate out the session handling from pre-connect handling
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
                                            // currently all other cases will deactivate the session
                                            let _session_pool  = Arc::clone(&session_pool);
                                            {
                                                // in some cases the session pool will not have the session
                                                _session_pool.write().await.deactivate(&session_id).await;
                                            }
                                        }
                                    }
                                    stream.write(&Packet::Disconnect(Disconnect::new(reason))).await?;
                                } else {
                                    // no active session to deactivate, shutdown the stream
                                    stream.shutdown().await?;
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
                _ = Broker::keep_alive_timer(keep_alive) => {
                   // disconnect the client and deactivate the session
                     if let Some(session) = &client_session {
                          let session = session.read().await;
                            if session.connected() {
                                // TODO send a disconnect packet to the client
                                // TODO send a will message
                                {
                                    let _session_pool  = Arc::clone(&session_pool);
                                   _session_pool.write().await.deactivate(session.id()).await;
                                }
                            }
                     } else {
                        // no active session to deactivate, shutdown the stream
                        stream.shutdown().await?;
                        return Ok(());
                     }
                }
                packet_result = stream.read() => {
                    match packet_result {
                        Ok(Some(packet)) => {
                            if !connected {
                                if let Some(session) = Broker::handle_pre_connect(packet, stream,  Arc::clone(&session_pool)).await? {
                                    {
                                        let mut session = session.write().await;
                                        keep_alive = session.keep_alive();
                                        session_control = session.take_control_receiver();
                                    }
                                    client_session = Some(Arc::clone(&session));
                                } else {
                                    continue;
                                }
                                connected = true;
                            } else if let Some(session) = &client_session {
                                Broker::handle_packet(packet, stream, Arc::clone(session), Arc::clone(&session_pool)).await?;
                            }
                            // TODO evaluate cases where connected and session may be None
                        }
                        Ok(None) => {
                            // handle the stream disconnect
                        }
                        Err(e) => {
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
        _session_pool: Arc<RwLock<SessionPool>>,
    ) -> Result<(), BrokerError> {
        match packet {
            Packet::PingRequest(_) => {
                let packet = PingResponse(FixedHeader::new(PacketType::PingResp));
                stream.write(&packet).await?;
                Ok(())
            }
            Packet::Disconnect(packet) => {
                if let Reason::Success = packet.reason {
                    // discard the will message
                    {
                        let mut session = session.write().await;
                        session.clear_will();
                    }
                } else {
                    // TODO send the will message
                }
                // deactivate the session
                {
                    let session_id = {
                        let session = session.read().await;
                        (*session).id().to_string()
                    };
                    let _session_pool = Arc::clone(&_session_pool);
                    _session_pool.write().await.deactivate(&session_id).await;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    async fn handle_control(
        control_channel: &mut Option<Receiver<SessionControl>>,
    ) -> Option<SessionControl> {
        loop {
            if let Some(ref mut rx) = control_channel {
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
        session_pool: Arc<RwLock<SessionPool>>,
    ) -> Result<Option<Arc<RwLock<Session>>>, BrokerError> {
        match packet {
            Packet::Connect(packet) => Broker::handle_connect(*packet, stream, session_pool).await,
            Packet::PingRequest(_packet) => {
                // allow clients without connected session to ping
                let packet = PingResponse(FixedHeader::new(PacketType::PingResp));
                stream.write(&packet).await?;
                Ok(None)
            }
            _ => {
                let header = Disconnect::new(Reason::ProtocolErr);
                let packet = Packet::Disconnect(header);
                stream.write(&packet).await?;
                Ok(None)
            }
        }
    }

    async fn handle_connect(
        connect: Connect,
        stream: &mut PacketStream,
        session_pool: Arc<RwLock<SessionPool>>,
    ) -> Result<Option<Arc<RwLock<Session>>>, BrokerError> {
        let session_id: String;
        let mut ack = ConnAck::default();
        let (default_keep_alive_secs, max_keep_alive_secs) = {
            let session_pool = session_pool.read().await;
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
        let session_expiry = if let Some(requested_expiry) = connect.session_expiry() {
            if requested_expiry > 0 {
                // set the ack expiry if the requested expiry is less greater than max allowed
                let max_expiry: u32;
                {
                    max_expiry = session_pool.read().await.session_expiry().as_secs() as u32;
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
            ack.properties_mut()
                .set_property(vaux_mqtt::property::Property::AssignedClientId(
                    session_id.clone(),
                ));
        } else {
            session_id = connect.client_id.clone();
        }
        let mut session_pool = session_pool.write().await;
        let session: Arc<RwLock<Session>> = if connect.clean_start {
            // clean start will remove any existing session if it exists and create a new session
            // if the session is active, the session will be taken over by the new client
            if let Some(existing_session) = session_pool.remove(session_id.as_str()) {
                let connected = { existing_session.read().await.connected() };
                if connected {
                    // send a disconnect packet to the client with reason TakenOver
                    let _ = Broker::handle_takeover(Arc::clone(&existing_session)).await;
                }
            }
            let mut new_session =
                Session::new(session_id.clone(), connect.will_message.clone(), keep_alive);
            new_session.set_connected(true);
            let new_session = Arc::new(RwLock::new(new_session));
            session_pool.add(Arc::clone(&new_session)).await;
            new_session
        } else {
            // activate existing session if in the inactive pool
            if let Some(session) = session_pool.activate(&session_id).await {
                {
                    let mut session = session.write().await;
                    ack.session_present = true;
                    // take over the session control channel
                    session.reset_control();
                }
                session
            } else if let Some(session) = session_pool.get_active(&session_id) {
                // take over an existing session if it is active and clean start is false
                {
                    // TODO handle errors
                    let _ = Broker::handle_takeover(Arc::clone(&session)).await;
                    let mut session = session.write().await;
                    ack.session_present = true;
                    // take over the session control channel
                    session.reset_control();
                    session.set_last_active();
                }
                session
            } else {
                // create a new session from the connect request
                let mut new_session =
                    Session::new(session_id.clone(), connect.will_message.clone(), keep_alive);
                new_session.set_connected(true);
                let new_session = Arc::new(RwLock::new(new_session));
                session_pool.add(Arc::clone(&new_session)).await;
                new_session
            }
        };
        drop(session_pool);
        // set the session expiration
        {
            let mut session = session.write().await;
            if let Some(expiry) = session_expiry {
                session.set_session_expiry(expiry);
            }
        }
        stream.write(&Packet::ConnAck(ack)).await?;
        Ok(Some(Arc::clone(&session)))
        //Ok(None)
    }

    async fn keep_alive_timer(keep_alive: Duration) {
        tokio::time::sleep(keep_alive).await;
    }

    async fn handle_takeover(session: Arc<RwLock<Session>>) -> Result<(), BrokerError> {
        // send a disconnect packet to the client with reason TakenOver
        session
            .read()
            .await
            .disconnect(Reason::SessionTakeOver)
            .await;
        // TODO wait for disconnect to complete from session state
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok(())
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
    /// Tests the default initialization behaviors for the broker. Changing the
    /// default behavior changes implicit contracts with clients and should be
    /// backwards compatible.
    fn test_default() {
        // tests would create false positives on contract behavior change
        // if the module level defaults were used to verify test results
        const EXPECTED_IP_ADDR: &str = "127.0.0.1";
        const EXPECTED_PORT: u16 = 1883;
        let broker = Broker::default();
        assert!(broker.config.listen_addr.is_ipv4(), "expected IPV4 address");
        assert_eq!(
            EXPECTED_IP_ADDR,
            broker.config.listen_addr.ip().to_string(),
            "expected local loopback address: 127.0.0.1"
        );
        assert_eq!(
            EXPECTED_PORT,
            broker.config.listen_addr.port(),
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
            broker.config.listen_addr.ip().to_string(),
            "expected local loopback address: 127.0.0.1"
        );
        assert_eq!(
            EXPECTED_PORT,
            broker.config.listen_addr.port(),
            "expected default listen port to be 1883"
        );
    }
}


