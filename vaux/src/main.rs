#[cfg(feature = "console")]
mod tui;

use clap::Parser;
use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::time::Duration;
use vaux_broker::config::{DEFAULT_LISTEN_ADDR, DEFAULT_PORT};
use vaux_broker::{Broker, Config};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about=None)]
struct Args {
    #[clap(short = 'l', long)]
    /// Listen address (default is "127.0.0.1")s
    listen_addr: Option<String>,
    #[clap(short, long)]
    port: Option<u16>,
    #[clap(
        short = 's',
        long,
        help = "maximum number of sessions including inactive"
    )]
    max_sessions: Option<u32>,
    #[clap(short = 'a', long, help = "maximum number of active sessions")]
    max_active_sessions: Option<u32>,
    #[clap(short = 'x', long, help = "session expiration in seconds")]
    session_expiration: Option<u32>,
    #[clap(short = 't', long, help = "default session timeout in seconds")]
    session_timeout: Option<u32>,
    #[clap(short = 'T', long, help = "maximum allowed session timeout in seconds")]
    max_session_timeout: Option<u32>,
    #[clap(short = 'U', long, help = "run terminal user interface")]
    tui: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let broker_version = env!("CARGO_PKG_VERSION");
    println!("{:-<1$}", "", 40);
    println!("-{:^38}-", format!("vaux MQTT broker v{}", broker_version));
    println!("{:-<1$}", "", 40);
    println!("\nCTRL-C to exit\n");

    let listen_addr = if let Some(listen_addr) = args.listen_addr {
        if let Ok(addr) = Ipv4Addr::from_str(&listen_addr) {
            addr
        } else {
            panic!("Listen address, \"{listen_addr}\" is not a valid IPV4 address",);
        }
    } else {
        Ipv4Addr::from_str(DEFAULT_LISTEN_ADDR).unwrap()
    };
    let listen_port = args.port.unwrap_or(DEFAULT_PORT);
    let listen_addr = SocketAddr::from((listen_addr, listen_port));
    let mut config = Config::new(listen_addr);
    if let Some(expiration) = args.session_expiration {
        config.session_expiry = Duration::from_secs(expiration as u64);
    }
    let mut broker = Broker::new_with_config(config);
    // TODO initialize from storage for long lived sessions
    let result = broker.run().await;
    println!("Broker started on: {listen_addr}");
    if let Err(e) = result {
        eprintln!("Error starting broker: {e}");
    }
}
