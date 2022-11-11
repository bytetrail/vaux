pub(crate) mod codec;
pub(crate) mod session;

use crate::broker::session::Session;
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio_util::codec::Framed;
use uuid::Uuid;
use vaux_mqtt::Packet::PingResponse;
use vaux_mqtt::{ConnAck, Disconnect, FixedHeader, MQTTCodecError, Packet, PacketType, Reason};

use self::codec::MQTTCodec;

pub const DEFAULT_PORT: u16 = 1883;
pub const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1";
const DEFAULT_MAX_KEEP_ALIVE: u64 = 60; // 60 seconds
const WAIT_IDLE_TIME: u64 = 50; // 50 milliseconds wait idle time
const KEEP_ALIVE_FACTOR: f32 = 1.5_f32;

#[derive(Debug, Clone)]
pub struct Broker {
    listen_addr: SocketAddr,
}

impl Default for Broker {
    /// Creates a new MQTT broker listening to local loopback on the default MQTT
    /// port (1883) for unsecure traffic
    fn default() -> Self {
        Broker {
            listen_addr: SocketAddr::try_from((
                Ipv4Addr::from_str(DEFAULT_LISTEN_ADDR).unwrap(),
                DEFAULT_PORT,
            ))
            .unwrap(),
        }
    }
}

impl Broker {
    /// Creates a new broker with the configuration specified. This method will
    /// not be used until the command line interface is developed. Remove the
    /// dead_code override when complete
    pub fn new(listen_addr: SocketAddr) -> Self {
        Broker { listen_addr }
    }

    pub async fn run(
        &mut self,
        session_pool: Arc<RwLock<HashMap<String, Arc<RwLock<Session>>>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match TcpListener::bind(self.listen_addr).await {
            Ok(listener) => {
                println!("broker accepting request on {:?}", self.listen_addr);
                loop {
                    let session_pool = session_pool.clone();
                    let (mut socket, _) = listener.accept().await?;
                    tokio::spawn(async move {
                        match Broker::handle_client(&mut socket, session_pool).await {
                            Ok(_) => {}
                            Err(e) => {
                                // TODO unhandled error in client handler should result in disconnect
                                eprintln!("error in child process: {}", e);
                            }
                        }
                    });
                }
            }
            Err(e) => {
                eprintln!("unable to start broker; error = {:?}", e);
                Err(Box::new(e))
            }
        }
    }

    async fn handle_client(
        stream: &mut TcpStream,
        session_pool: Arc<RwLock<HashMap<String, Arc<RwLock<Session>>>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let session_id: String;
        let mut framed = Framed::new(stream, MQTTCodec {});
        let idle = Duration::from_millis(WAIT_IDLE_TIME);
        let session = match framed.next().await {
            Some(Ok(Packet::Connect(packet))) => {
                let mut active_session: Option<Arc<RwLock<Session>>> = None;
                let mut ack = ConnAck::default();
                // handle the client id
                if packet.client_id.len() == 0 {
                    session_id = Uuid::new_v4().to_string();
                    ack.assigned_client_id = Some(session_id.clone());
                } else {
                    session_id = packet.client_id;
                }
                if let Some(session) = session_pool.read().await.get(&session_id) {
                    active_session = Some(session.clone());
                    ack.session_present = true;
                } 
                if packet.clean_start || active_session.is_none() {
                    let mut session = Session::new(
                        session_id.clone(),
                        Duration::from_secs(DEFAULT_MAX_KEEP_ALIVE),
                    );
                    let session = Arc::new(RwLock::new(session));
                    session_pool
                        .write()
                        .await
                        .insert(session_id.clone(), session.clone());
                    active_session = Some(session);
                }
                if packet.keep_alive > DEFAULT_MAX_KEEP_ALIVE as u16 {
                    ack.server_keep_alive = Some(DEFAULT_MAX_KEEP_ALIVE as u16);
                } else {
                    let mut session = active_session
                        .as_ref()
                        .unwrap()
                        .write()
                        .await;
                        if let Some(expiry) = packet.session_expiry_interval {
                            session.expiry = Duration::from_secs(expiry as u64);
                        }
                        session.set_max_keep_alive_secs(packet.keep_alive as u64);
                }
                framed.send(Packet::ConnAck(ack)).await?;
                active_session
            }
            Some(Ok(Packet::PingRequest(_packet))) => {
                // allow clients without connected session to ping
                let resp = PingResponse(FixedHeader::new(PacketType::PingResp));
                framed.send(resp).await?;
                None
            }
            _ => {
                let mut header = Disconnect::new(Reason::ProtocolErr);
                let disconnect = Packet::Disconnect(header);

                framed.send(disconnect).await?;
                return Err(Box::new(MQTTCodecError::new("connect packet not received")));
            }
        };
        if let Some(session) = session {
            let keep_alive = Duration::from_secs(
                (session.read().await.max_keep_alive_secs() as f32 * KEEP_ALIVE_FACTOR) as u64,
            );
            let mut last_activity = Instant::now();
            loop {
                let request = framed.next().await;
                if let Some(request) = request {
                    last_activity = Instant::now();
                    match request {
                        Ok(request) => match request {
                            Packet::PingRequest(_) => {
                                let header = FixedHeader::new(PacketType::PingResp);
                                framed.send(Packet::PingResponse(header)).await?;
                            }
                            req => {
                                println!("Packet request {:?}", req);
                                return Err(Box::new(MQTTCodecError::new(
                                    format!("unsupported packet type: {:?}", req).as_str(),
                                )));
                            }
                        },
                        Err(e) => {
                            return Err(Box::new(e));
                        }
                    } // match request
                } // if let Some(request ...
                std::thread::sleep(idle);
                if last_activity.elapsed() > keep_alive {
                    // immediate disconnect
                    // TODO review specification for normal disconnect packet
                    break;
                }
            } // loop
        }
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
