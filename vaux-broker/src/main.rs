mod broker;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use broker::Broker;
use crate::broker::session::{Session};
use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about=None)]
struct Args {
    #[clap(short, long)]
    port: Option<u16>

}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let broker_version = env!("CARGO_PKG_VERSION");
    println!("{:-<1$}", "", 40);
    println!(
        "-{:^38}-",
        format!("vaux-broker MQTT broker v{}", broker_version)
    );
    println!("{:-<1$}", "", 40);
    println!("\nCTRL-C to exit\n");

    let mut broker = Broker::default();
    // TODO initialize from storage for long lived sessions
    let session_pool: Arc<RwLock<HashMap<String, Arc<RwLock<Session>>>>> = Arc::new(RwLock::new(HashMap::new()));
    let _ = broker.run(session_pool).await;
}
