mod broker;

use broker::Broker;

#[tokio::main]
async fn main() {
    let broker_version = env!("CARGO_PKG_VERSION");
    println!("{:-<1$}", "", 40);
    println!(
        "-{:^38}-",
        format!("vaux-broker MQTT broker v{}", broker_version)
    );
    println!("{:-<1$}", "", 40);
    println!("\nCTRL-C to exit\n");

    let mut broker = Broker::default();
    let _ = broker.run().await;
}
