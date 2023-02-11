pub(crate) mod codec;
pub(crate) mod session;

use crate::broker::session::Session;
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio::time::timeout;
use tokio_util::codec::Framed;
use uuid::Uuid;
use vaux_mqtt::Packet::PingResponse;
use vaux_mqtt::{ConnAck, Disconnect, FixedHeader, MQTTCodecError, Packet, PacketType, Reason};

use self::codec::MQTTCodec;

pub const DEFAULT_PORT: u16 = 1883;
pub const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1";
const DEFAULT_KEEP_ALIVE: u64 = 30; // 60 seconds

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
                    let mut session_lock = session.blocking_write();
                    if session_lock.connected() {
                        // handle take over
                        session_lock.set_orphaned();
                    } else if !session_lock.orphaned() {
                        session_lock.set_connected(true);
                        active_session = Some(session.clone());
                        ack.session_present = true;
                    }
                }
                if packet.clean_start || active_session.is_none() {
                    let session =
                        Session::new(session_id.clone(), Duration::from_secs(DEFAULT_KEEP_ALIVE));
                    let session = Arc::new(RwLock::new(session));
                    session_pool
                        .write()
                        .await
                        .insert(session_id.clone(), session.clone());
                    active_session = Some(session);
                }
                if packet.keep_alive > DEFAULT_KEEP_ALIVE as u16 {
                    ack.server_keep_alive = Some(DEFAULT_KEEP_ALIVE as u16);
                } else {
                    let mut session = active_session.as_ref().unwrap().write().await;
                    if let Some(expiry) = packet.session_expiry_interval {
                        session.session_expiry = Duration::from_secs(expiry as u64);
                    }
                    session.set_keep_alive(packet.keep_alive as u64);
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
                let header = Disconnect::new(Reason::ProtocolErr);
                let disconnect = Packet::Disconnect(header);
                framed.send(disconnect).await?;
                return Err(Box::new(MQTTCodecError::new("connect packet not received")));
            }
        };
        if let Some(session) = session {
            loop {
                let duration = Duration::from_secs(session.read().await.keep_alive());
                match timeout(duration, framed.next()).await {
                    Ok(request) => {
                        if session.read().await.orphaned() {
                            // respond with session taken over error
                            // TODO
                        }
                        if let Some(request) = request {
                            session.write().await.set_last_active();
                            match request {
                                Ok(request) => match request {
                                    Packet::PingRequest(_) => {
                                        let header = FixedHeader::new(PacketType::PingResp);
                                        framed.send(Packet::PingResponse(header)).await?;
                                    }
                                    Packet::Disconnect(_) => {
                                        // exit loop closing connection
                                        session.write().await.set_connected(false);
                                        break;
                                    }
                                    req => {
                                        return Err(Box::new(MQTTCodecError::new(
                                            format!("unexpected packet type: {:?}", req).as_str(),
                                        )));
                                    }
                                },
                                Err(e) => {
                                    session.write().await.set_connected(false);
                                    // disconnect with protocol error
                                    let disconnect = Disconnect::new(Reason::ProtocolErr);
                                    framed.send(Packet::Disconnect(disconnect)).await?;
                                    return Err(Box::new(e));
                                }
                            } // match request
                        } // if let Some(request ...
                    }
                    Err(_elapsed) => {
                        // connection keep alive expired
                        session.write().await.set_connected(false);
                        let disconnect = Disconnect::new(Reason::KeepAliveTimeout);
                        framed.send(Packet::Disconnect(disconnect)).await?;
                        break;
                    }
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
