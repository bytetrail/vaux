use futures::{SinkExt, StreamExt};
use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;
use vaux_mqtt::Packet::PingResponse;
use vaux_mqtt::{ConnAck, FixedHeader, MQTTCodec, MQTTCodecError, Packet, PacketType, Reason};

const DEFAULT_PORT: u16 = 1883;
const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1";

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

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match TcpListener::bind(self.listen_addr).await {
            Ok(listener) => {
                println!("broker accepting request on {:?}", self.listen_addr);
                loop {
                    let (mut socket, _) = listener.accept().await?;
                    tokio::spawn(async move {
                        match Broker::handle_client(&mut socket).await {
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

    async fn handle_client(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
        let mut loop_count = 0;
        let mut framed = Framed::new(stream, MQTTCodec {});
        let packet = match framed.next().await {
            Some(Ok(Packet::Connect(packet))) => {
                let ack = ConnAck::default();
                framed.send(Packet::ConnAck(ack)).await?;
                Some(packet)
            }
            Some(Ok(Packet::PingRequest(_packet))) => {
                let resp = PingResponse(FixedHeader::new(PacketType::PingResp));
                framed.send(resp).await?;
                None
            }
            _ => {
                println!("connect not received");
                return Err(Box::new(MQTTCodecError::new("connect packet not received")));
            }
        };
        if packet.is_some() {
            loop {
                println!("Frame {}", loop_count);
                let request = framed.next().await;
                if let Some(request) = request {
                    match request {
                        Ok(request) => match request {
                            Packet::PingRequest(_) => {
                                let header = FixedHeader::new(PacketType::PingResp);
                                framed.send(Packet::PingResponse(header)).await?;
                            }
                            req => {
                                println!("unsupported packet type {:?}", &req);
                                return Err(Box::new(MQTTCodecError::new(
                                    format!("unsupported packet type: {:?}", req).as_str(),
                                )));
                            }
                        },
                        Err(e) => {
                            println!("error handling client {:?}", e);
                            return Err(Box::new(e));
                        }
                    }
                } else {
                    println!(" nothing ? ");
                }
                loop_count += 1;
            }
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
