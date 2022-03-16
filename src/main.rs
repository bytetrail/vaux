mod broker;



#[tokio::main]
async fn main() {
    println!("Hello, world!");

    let mut broker = broker::Broker::default();
    broker.run().await;
}
