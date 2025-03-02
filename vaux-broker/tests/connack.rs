use std::{net::SocketAddr, time::Duration};

use vaux_broker::{Broker, Config};
use vaux_client::MqttConnection;

#[tokio::test]
async fn server_assigned_keep_alive() {
    const TEST_PORT: u16 = 8385;
    const TEST_ADDR: &str = "127.0.0.1";
    const CONNECT_TIMEOUT: u64 = 5000;

    let mut broker = Broker::new_with_config(Config {
        listen_addr: SocketAddr::new(TEST_ADDR.parse().unwrap(), TEST_PORT),
        default_keep_alive: Duration::from_secs(30),
        max_keep_alive: Duration::from_secs(30),
        session_expiry: Duration::from_secs(60 * 10),
    });
    broker.run().await.expect("failed to start broker");
    let mut client = vaux_client::ClientBuilder::new(
        MqttConnection::new()
            .with_host(TEST_ADDR)
            .with_port(TEST_PORT),
    )
    .with_client_id("connack_client")
    .with_auto_ack(true)
    .with_keep_alive(Duration::from_secs(120))
    .build()
    .expect("failed to create client");
    let client_handle = client
        .try_start(Duration::from_millis(CONNECT_TIMEOUT), true)
        .await
        .expect("failed to start client");
    let assigned_keep_alive = client.keep_alive().await;
    assert_eq!(assigned_keep_alive, Duration::from_secs(30));
    client.stop().await.expect("failed to stop client");
    broker.stop().await;
    let _ = client_handle.await.expect("failed to stop client");
}

#[tokio::test]
async fn server_assigned_expiry() {}

#[tokio::test]
async fn server_assigned_client_id() {}
