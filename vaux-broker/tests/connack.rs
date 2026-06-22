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
        ..Default::default()
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
    .await
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
async fn server_assigned_expiry() {
    const TEST_PORT: u16 = 8386;
    const TEST_ADDR: &str = "127.0.0.1";
    const CONNECT_TIMEOUT: u64 = 5000;
    const SERVER_SESSION_EXPIRY: Duration = Duration::from_secs(60 * 10);
    const REQUESTED_SESSION_EXPIRY: Duration = Duration::from_secs(60 * 30);

    let mut broker = Broker::new_with_config(Config {
        listen_addr: SocketAddr::new(TEST_ADDR.parse().unwrap(), TEST_PORT),
        default_keep_alive: Duration::from_secs(30),
        max_keep_alive: Duration::from_secs(30),
        session_expiry: SERVER_SESSION_EXPIRY,
        ..Default::default()
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
    .with_session_expiry(REQUESTED_SESSION_EXPIRY)
    .build()
    .await
    .expect("failed to create client");
    let client_handle = client
        .try_start(Duration::from_millis(CONNECT_TIMEOUT), true)
        .await
        .expect("failed to start client");
    let assigned_expiry = client.session_expiry().await;
    assert_eq!(assigned_expiry, SERVER_SESSION_EXPIRY);
    client.stop().await.expect("failed to stop client");
    broker.stop().await;
    let _ = client_handle.await.expect("failed to stop client");
}

#[tokio::test]
async fn server_assigned_client_id() {}

#[tokio::test]
async fn max_active_sessions_exceeded() {
    const TEST_PORT: u16 = 8387;
    const TEST_ADDR: &str = "127.0.0.1";
    const CONNECT_TIMEOUT: u64 = 5000;

    let mut broker = Broker::new_with_config(Config {
        listen_addr: SocketAddr::new(TEST_ADDR.parse().unwrap(), TEST_PORT),
        default_keep_alive: Duration::from_secs(30),
        max_keep_alive: Duration::from_secs(30),
        session_expiry: Duration::from_secs(60),
        max_active_sessions: Some(1),
        ..Default::default()
    });
    broker.run().await.expect("failed to start broker");

    // first client connects — should succeed
    let mut client_one = vaux_client::ClientBuilder::new(
        MqttConnection::new()
            .with_host(TEST_ADDR)
            .with_port(TEST_PORT),
    )
    .with_client_id("max-client-1")
    .with_auto_ack(true)
    .with_keep_alive(Duration::from_secs(30))
    .build()
    .await
    .expect("failed to create client one");

    let _handle_one = client_one
        .try_start(Duration::from_millis(CONNECT_TIMEOUT), true)
        .await
        .expect("client one should connect successfully");

    // second client connects — should be rejected (max_active_sessions = 1)
    let mut client_two = vaux_client::ClientBuilder::new(
        MqttConnection::new()
            .with_host(TEST_ADDR)
            .with_port(TEST_PORT),
    )
    .with_client_id("max-client-2")
    .with_auto_ack(true)
    .with_keep_alive(Duration::from_secs(30))
    .build()
    .await
    .expect("failed to create client two");

    let result = client_two
        .try_start(Duration::from_millis(CONNECT_TIMEOUT), true)
        .await;
    assert!(result.is_err(), "client two should be rejected");

    // disconnect client one, then client two should be able to connect
    client_one.stop().await.expect("failed to stop client one");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut client_three = vaux_client::ClientBuilder::new(
        MqttConnection::new()
            .with_host(TEST_ADDR)
            .with_port(TEST_PORT),
    )
    .with_client_id("max-client-3")
    .with_auto_ack(true)
    .with_keep_alive(Duration::from_secs(30))
    .build()
    .await
    .expect("failed to create client three");

    let _handle_three = client_three
        .try_start(Duration::from_millis(CONNECT_TIMEOUT), true)
        .await
        .expect("client three should connect after client one disconnected");

    broker.stop().await;
}
