mod broker;

use crate::broker::session::Session;
use crate::broker::{DEFAULT_LISTEN_ADDR, DEFAULT_PORT};
use broker::Broker;
use clap::Parser;
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about=None)]
struct Args {
    #[clap(short = 'h', long)]
    /// Listen address (default is "127.0.0.1")s
    listen_addr: Option<String>,
    #[clap(short, long)]
    port: Option<u16>,
    #[clap(short = 's', long)]
    /// Maximum number of sessions active/in-use
    max_sessions: Option<u32>,
    #[clap(short = 'a', long)]
    max_active_sessions: Option<u32>,
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
            panic!(
                "Listen address, \"{}\" is not a valid IPV4 address",
                listen_addr
            );
        }
    } else {
        Ipv4Addr::from_str(DEFAULT_LISTEN_ADDR).unwrap()
    };
    let listen_port = args.port.or(Some(DEFAULT_PORT)).unwrap();
    let listen_addr = SocketAddr::try_from((listen_addr, listen_port)).unwrap();

    let mut broker = Broker::new(listen_addr);
    // TODO initialize from storage for long lived sessions
    let session_pool: Arc<RwLock<HashMap<String, Arc<RwLock<Session>>>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let _ = broker.run(session_pool).await;
}
