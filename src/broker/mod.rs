use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const DEFAULT_PORT: u16 = 1883;
const DEFAULT_LISTEN_ADDR: &'static str = "127.0.0.1";

pub struct Broker {
    listen_addr: SocketAddr,
}

impl Default for Broker {
    fn default() -> Self {
        Broker {
            listen_addr: SocketAddr::try_from((Ipv4Addr::from_str(
                DEFAULT_LISTEN_ADDR).unwrap(), DEFAULT_PORT)).unwrap()
        }
    }
}

impl Broker {
    // Creates a new broker with the configuration specified
    pub fn new(listen_addr: SocketAddr) -> Self {
        Broker {
            listen_addr,
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(self.listen_addr).await?;
        loop {
            println!("Running!");
            let (mut socket, _) = listener.accept().await?;
            tokio::spawn(async move {
                let mut buf = [0; 1024];
                loop {
                    let count = match socket.read(&mut buf).await {
                        Ok(count) if count == 0 => return,
                        Ok(count) => count,
                        Err(e) => {
                            eprintln!("failed to read from socket; error = {:?}", e);
                            return;
                        }
                    };
                    // write the data in the buffer back out
                    if let Err(e) = socket.write_all(&buf[0..count]).await {
                        eprintln!("failed to write data to socket; error = {:?}", e);
                    }
                }
            });
        }
    }
}

