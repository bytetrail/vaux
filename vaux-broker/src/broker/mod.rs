pub(crate) mod codec;
pub(crate) mod config;
pub(crate) mod session;

use crate::broker::session::Session;
use session::SessionPool;

use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;

use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::select;
use tokio::sync::RwLock;
use uuid::Uuid;
use vaux_async::stream::{packet, AsyncMqttStream, MqttStream, PacketStream};
use vaux_mqtt::Packet::PingResponse;
use vaux_mqtt::{
    ConnAck, Connect, Disconnect, FixedHeader, MqttCodecError, Packet, PacketType, Reason,
};

pub const DEFAULT_PORT: u16 = 1883;
pub const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1";
const DEFAULT_KEEP_ALIVE: Duration = Duration::from_secs(30);
const INIT_STREAM_BUFFER_SIZE: usize = 4096;

pub enum BrokerError {
    CodecError(MqttCodecError),
    IoError(std::io::Error),
    StreamError(vaux_async::stream::Error),
}

impl std::fmt::Display for BrokerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            BrokerError::CodecError(e) => write!(f, "codec error: {}", e),
            BrokerError::IoError(e) => write!(f, "io error: {}", e),
            BrokerError::StreamError(e) => write!(f, "stream error {}", e),
        }
    }
}

impl From<std::io::Error> for BrokerError {
    fn from(e: std::io::Error) -> Self {
        BrokerError::IoError(e)
    }
}

impl From<MqttCodecError> for BrokerError {
    fn from(e: MqttCodecError) -> Self {
        BrokerError::CodecError(e)
    }
}

impl From<vaux_async::stream::Error> for BrokerError {
    fn from(e: vaux_async::stream::Error) -> Self {
        BrokerError::StreamError(e)
    }
}

#[derive(Debug)]
pub struct Broker {
    listen_addr: SocketAddr,
    session_pool: Arc<RwLock<SessionPool>>,
}

impl Default for Broker {
    /// Creates a new MQTT broker listening to local loopback on the default MQTT
    /// port (1883) for unsecure traffic
    fn default() -> Self {
        Broker {
            listen_addr: SocketAddr::from((
                Ipv4Addr::from_str(DEFAULT_LISTEN_ADDR).unwrap(),
                DEFAULT_PORT,
            )),
            session_pool: Arc::new(RwLock::new(SessionPool::default())),
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

    pub async fn run(&mut self) -> Result<(), BrokerError> {
        match TcpListener::bind(self.listen_addr).await {
            Ok(listener) => {
                println!("broker accepting request on {:?}", self.listen_addr);
                loop {
                    println!("broker waiting for connection");
                    match listener.accept().await {
                        Ok((socket, _)) => {
                            println!("broker accepted connection from {:?}", socket.peer_addr());
                            // create an mqtt stream from the tcp stream
                            let mut stream = PacketStream::new(
                                AsyncMqttStream(MqttStream::TcpStream(socket)),
                                Some(INIT_STREAM_BUFFER_SIZE),
                                None,
                            );
                            let session_pool = self.session_pool.clone();
                            tokio::spawn(async move {
                                Broker::handle_client(&mut stream, session_pool).await
                            });
                        }
                        Err(e) => {
                            println!("broker error accepting connection: {}", e);
                            return Err(e.into());
                        }
                    }
                }
            }
            Err(e) => {
                return Err(e.into());
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
                          let mut session = session.read().await;
                            if session.connected() {
                                // TODO send a disconnect packet to the client
                                // TODO send a will message
                                session_pool.write().await.deactivate(&session.id()).await;
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
                                    client_session = Some(Arc::clone(&session));
                                    let session = session.read().await;
                                    keep_alive = session.keep_alive();
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
        packet: Connect,
        stream: &mut PacketStream,
        session_pool: Arc<RwLock<SessionPool>>,
    ) -> Result<Option<Arc<RwLock<Session>>>, BrokerError> {
        let session_id: String;
        let mut ack = ConnAck::default();
        // handle the client id
        if packet.client_id.is_empty() {
            session_id = Uuid::new_v4().to_string();
            ack.properties_mut()
                .set_property(vaux_mqtt::property::Property::AssignedClientId(
                    session_id.clone(),
                ));
        } else {
            session_id = packet.client_id.clone();
        }

        // write lock on session pool
        let mut session_pool = session_pool.write().await;
        let session: Arc<RwLock<Session>> =
            if let Some(session) = session_pool.activate(&session_id).await {
                session
            } else {
                // if the session is active, take over the session by disconnected the existing session
                // and cloning properties for a new session if clean-start is false
                if let Some(session) = session_pool.remove_active(&session_id) {
                    {
                        let mut session = session.write().await;
                        Broker::handle_takeover(stream).await;
                        if !packet.clean_start {
                            session.clear();
                        }
                    }
                    session
                } else {
                    // create a new session from the connect request
                    let new_session = Arc::new(RwLock::new(Session::new_from_connect(packet)));
                    session_pool.add(&new_session).await;
                    new_session
                }
            };
        stream.write(&Packet::ConnAck(ack)).await?;
        Ok(Some(session))
    }

    async fn keep_alive_timer(keep_alive: Duration) {
        tokio::time::sleep(keep_alive).await;
    }

    pub async fn handle_takeover(packet_stream: &mut PacketStream) {
        // send a disconnect packet to the client with reason TakenOver
        packet_stream
            .write(&vaux_mqtt::Packet::Disconnect(vaux_mqtt::Disconnect::new(
                vaux_mqtt::Reason::SessionTakeOver,
            )))
            .await
            .unwrap();
        packet_stream.shutdown().await.unwrap();
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
