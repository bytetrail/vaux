pub(crate) mod codec;
pub(crate) mod config;
pub(crate) mod session;

use session::Session;
use session::SessionControl;
use session::SessionPool;
use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;

use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::select;
use tokio::sync::RwLock;
use uuid::Uuid;
use vaux_async::stream::{AsyncMqttStream, MqttStream, PacketStream};
use vaux_mqtt::Packet::PingResponse;
use vaux_mqtt::{
    ConnAck, Connect, Disconnect, FixedHeader, MqttCodecError, Packet, PacketType, Reason,
};

pub const DEFAULT_PORT: u16 = 1883;
pub const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1";
const DEFAULT_KEEP_ALIVE_SECS: u16 = 30;
const MAX_KEEP_ALIVE_AS_SECS: u16 = 120;
const BROKER_KEEP_ALIVE_FACTOR: f32 = 1.5;
const DEFAULT_KEEP_ALIVE: Duration =
    Duration::from_secs((DEFAULT_KEEP_ALIVE_SECS as f32 * BROKER_KEEP_ALIVE_FACTOR) as u64);
const MAX_KEEP_ALIVE: Duration =
    Duration::from_secs((MAX_KEEP_ALIVE_AS_SECS as f32 * BROKER_KEEP_ALIVE_FACTOR) as u64);
// 10 minute session expiration
const DEFAULT_SESSION_EXPIRY: Duration = Duration::from_secs(60 * 10);
const INIT_STREAM_BUFFER_SIZE: usize = 4096;

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
    listen_addr: SocketAddr,
    session_pool: Arc<RwLock<SessionPool>>,
    command_channel: (Sender<BrokerCommand>, Option<Receiver<BrokerCommand>>),
}

impl Default for Broker {
    /// Creates a new MQTT broker listening to local loopback on the default MQTT
    /// port (1883) for unsecure traffic    
    fn default() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        Broker {
            listen_addr: SocketAddr::from((
                Ipv4Addr::from_str(DEFAULT_LISTEN_ADDR).unwrap(),
                DEFAULT_PORT,
            )),
            session_pool: Arc::new(RwLock::new(SessionPool::new_with_expiry(
                DEFAULT_SESSION_EXPIRY,
            ))),
            command_channel: (sender, Some(receiver)),
        }
    }
}

impl Broker {
    /// Creates a new broker with the configuration specified. This method will
    /// not be used until the command line interface is developed. Remove the
    /// dead_code override when complete
    pub fn new(listen_addr: SocketAddr) -> Self {
        Broker {
            listen_addr,
            ..Default::default()
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
        println!("broker accepting request on {:?}", self.listen_addr);
        let session_pool = Arc::clone(&self.session_pool);
        match TcpListener::bind(self.listen_addr).await {
            Ok(listener) => {
                let mut command = self.command_channel.1.take().unwrap();
                let handle: JoinHandle<Result<(), BrokerError>> = tokio::spawn(async move {
                    loop {
                        select! {
                            cmd = command.recv() => {
                                match cmd {
                                    Some(BrokerCommand::Stop) => {
                                        println!("broker stopping");
                                        return Ok(());
                                    }
                                    Some(BrokerCommand::PauseAccept) => {
                                        println!("broker pausing accept");
                                        // TODO pause accept
                                    }
                                    Some(BrokerCommand::ResumeAccept) => {
                                        println!("broker resuming accept");
                                        // TODO resume accept
                                    }
                                    Some(BrokerCommand::SetSessionExpiry(expiration)) => {
                                        session_pool.write().await.set_session_expiry(expiration);
                                    }
                                    None => {
                                        println!("broker command channel closed");
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
            Err(e) => {
                println!("broker error binding to address: {}", e);
                Err(e.into())
            }
        }
    }

    async fn handle_client(
        stream: &mut PacketStream,
        session_pool: Arc<RwLock<SessionPool>>,
    ) -> Result<(), BrokerError> {
        let mut connected = false;
        let mut keep_alive = DEFAULT_KEEP_ALIVE;
        let mut client_session: Option<Arc<RwLock<Session>>> = None;
        loop {
            // TODO separate out the session handling from pre-connect handling
            select! {
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
                                if let Some(session) = Broker::handle_pre_connect(packet, stream, Arc::clone(&session_pool)).await? {
                                    {
                                        let session = session.read().await;
                                        keep_alive = session.keep_alive();
                                        println!("session keep alive: {:?}", keep_alive);
                                    }
                                    client_session = Some(Arc::clone(&session));
                                } else {
                                    continue;
                                }
                                connected = true;
                            }
                        }
                        Ok(None) => {
                            // handle the disconnect
                        }
                        Err(e) => {
                            return Err(e.into());
                        }
                    }
                }
            }
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
        println!("connect: {:?}", connect);
        let session_id: String;
        let mut ack = ConnAck::default();
        // handle keep alive request
        let keep_alive = if connect.keep_alive < DEFAULT_KEEP_ALIVE_SECS {
            ack.set_server_keep_alive(DEFAULT_KEEP_ALIVE.as_secs() as u16);
            DEFAULT_KEEP_ALIVE
        } else if connect.keep_alive > MAX_KEEP_ALIVE_AS_SECS {
            ack.set_server_keep_alive(MAX_KEEP_ALIVE_AS_SECS);
            MAX_KEEP_ALIVE
        } else {
            Duration::from_secs((connect.keep_alive as f32 * BROKER_KEEP_ALIVE_FACTOR) as u64)
        };
        // handle the session expiry request
        let session_expiry = if let Some(requested_expiry) = connect.session_expiry() {
            println!("requested session expiry: {}", requested_expiry);
            if requested_expiry > 0 {
                println!("requested session expiry > 0: {}", requested_expiry);
                // set the ack expiry if the requested expiry is less greater than max allowed
                let mut max_expiry = 0;
                {
                    println!("read session pool for max expiry");
                    max_expiry = session_pool.read().await.session_expiry().as_secs() as u32;
                    println!("max session expiry: {}", max_expiry);
                }
                if requested_expiry > max_expiry {
                    println!("requested session expiry > max: {}", requested_expiry);
                    ack.set_session_expiry(max_expiry);
                    Some(Duration::from_secs(max_expiry as u64))
                } else {
                    println!(
                        "granted requested session expiry <= max: {}",
                        requested_expiry
                    );
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
        let session: Arc<RwLock<Session>> =
            if let Some(session) = session_pool.activate(&session_id).await {
                println!("session: {:?} activated", session_id);
                if connect.clean_start {
                    // if the session is active and clean-start is true, clear the session
                    let mut session = session.write().await;
                    session.clear();
                } else {
                    ack.session_present = true;
                }
                session
            } else {
                println!("session: {:?} not found in deactivated", session_id);
                // if the session is active, take over the session by disconnecting the existing session
                // and cloning properties for a new session if clean-start is false
                if let Some(session) = session_pool.remove_active(&session_id) {
                    println!("session: {} taken over", session_id);
                    {
                        let mut session = session.write().await;
                        Broker::handle_takeover(stream).await;
                        if connect.clean_start {
                            session.clear();
                        } else {
                            ack.session_present = true;
                        }
                    }
                    session
                } else {
                    println!("created session: {:?}", session_id);
                    // create a new session from the connect request
                    let new_session =
                        Session::new(session_id.clone(), connect.will_message.clone(), keep_alive);
                    println!("new session: {:?}", new_session);

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
        println!("session: {:?} connected", session_id);
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
        tokio::time::sleep(Duration::from_millis(250)).await;
        Ok(())
    }
}

#[cfg(test)]
mod test {
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
        assert!(broker.listen_addr.is_ipv4(), "expected IPV4 address");
        assert_eq!(
            EXPECTED_IP_ADDR,
            broker.listen_addr.ip().to_string(),
            "expected local loopback address: 127.0.0.1"
        );
        assert_eq!(
            EXPECTED_PORT,
            broker.listen_addr.port(),
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

        let broker = Broker::new(listen_addr);
        assert_eq!(
            EXPECTED_IP_ADDR,
            broker.listen_addr.ip().to_string(),
            "expected local loopback address: 127.0.0.1"
        );
        assert_eq!(
            EXPECTED_PORT,
            broker.listen_addr.port(),
            "expected default listen port to be 1883"
        );
    }
}
