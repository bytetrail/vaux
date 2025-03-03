use std::time::Duration;
use tokio::task::JoinHandle;
use vaux_client::PacketChannel;

const TOPIC: &str = "hello-vaux";
const MESSAGE: &str = "hello world";
const HOST: &str = "127.0.0.1";
const PORT: u16 = 1883;

#[tokio::main]
async fn main() {
    let connection = vaux_client::MqttConnection::new();
    let connection = connection.with_host(HOST).with_port(PORT);

    let mut client = vaux_client::ClientBuilder::new(connection)
        .with_auto_ack(true)
        .with_auto_packet_id(true)
        .with_receive_max(10)
        .with_session_expiry(10000)
        .with_keep_alive(Duration::from_secs(30))
        .with_max_connect_wait(Duration::from_secs(5))
        .build()
        .await
        .unwrap();

    let handle: Option<JoinHandle<_>> =
        match client.try_start(Duration::from_millis(5000), true).await {
            Ok(h) => Some(h),
            Err(e) => {
                eprintln!("unable to start client: {:?}", e);
                return;
            }
        };
    client.publish_str(TOPIC, MESSAGE).await.unwrap();
    client.stop().await.unwrap();
    if let Some(h) = handle {
        if h.await.is_err() {
            eprintln!("error in client task");
        }
    }
}
