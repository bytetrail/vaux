use std::time::Duration;
use vaux_client::{ErrorKind, MqttConnection};

#[tokio::test]
pub async fn basic_connect() {
    let listen_addr = "127.0.0.1:8383";
    let mut broker = vaux_broker::Broker::new_with_config(vaux_broker::Config {
        listen_addr: listen_addr.parse().unwrap(),
        default_keep_alive: Duration::from_secs(30),
        max_keep_alive: Duration::from_secs(30),
        session_expiry: Duration::from_secs(60 * 10),
        ..Default::default()
    });
    let result = broker.run().await;
    assert!(result.is_ok());

    let client = vaux_client::ClientBuilder::new(
        MqttConnection::new().with_host("127.0.0.1").with_port(8383),
    )
    .with_client_id("client_id")
    .with_auto_ack(true)
    .with_keep_alive(Duration::from_secs(30))
    .build()
    .await;
    if let Ok(mut client) = client {
        let result = client.try_start(Duration::from_millis(1500), true).await;
        assert!(result.is_ok());
    } else {
        panic!("Failed to create client");
    }
    broker.stop().await;
}

#[tokio::test]
pub async fn connect_with_takeover() {
    const TAKEOVER_CLIENT_ID: &str = "takeover_client_id";
    const SESSION_EXPIRY: Duration = Duration::from_secs(60 * 10);
    const CONNECT_TIMEOUT: u64 = 5000;
    const TEST_PORT: u16 = 8384;
    let listen_addr = "127.0.0.1:8384";
    let mut broker = vaux_broker::Broker::new_with_config(vaux_broker::Config {
        listen_addr: listen_addr.parse().unwrap(),
        default_keep_alive: Duration::from_secs(30),
        max_keep_alive: Duration::from_secs(30),
        session_expiry: Duration::from_secs(60 * 10),
        ..Default::default()
    });
    let result = broker.run().await;
    assert!(result.is_ok());

    let client_one = vaux_client::ClientBuilder::new(
        MqttConnection::new()
            .with_host("127.0.0.1")
            .with_port(TEST_PORT),
    )
    .with_client_id(TAKEOVER_CLIENT_ID)
    .with_auto_ack(true)
    .with_session_expiry(SESSION_EXPIRY)
    .build()
    .await;

    println!("Client one created");
    if let Err(e) = client_one {
        panic!("Failed to create client one: {:?}", e);
    }
    let mut client_one = client_one.unwrap();

    let client_one_handle = client_one
        .try_start(Duration::from_millis(CONNECT_TIMEOUT), true)
        .await
        .expect("failed to start client");

    let mut client_two = vaux_client::ClientBuilder::new(
        MqttConnection::new()
            .with_host("127.0.0.1")
            .with_port(TEST_PORT),
    )
    .with_client_id(TAKEOVER_CLIENT_ID)
    .with_auto_ack(true)
    .with_session_expiry(SESSION_EXPIRY)
    .build()
    .await
    .expect("failed to create client");

    let _client_two_handle = client_two
        .try_start(Duration::from_millis(CONNECT_TIMEOUT), true)
        .await
        .expect("failed to start client");
    // get the client two packet out receiver
    let mut consumer = client_two
        .take_packet_consumer()
        .expect("failed to get packet out receiver");
    // PING using client two and verify PINGRESP
    client_two.ping().await.expect("failed to ping");
    let packet = consumer.recv().await.expect("failed to receive packet");
    match packet {
        vaux_mqtt::Packet::PingResponse(_) => {
            // PINGRESP received
            println!("Received PINGRESP");
        }
        _ => panic!("Expected PINGRESP"),
    }
    broker.stop().await;
    let result = client_one_handle.await;
    assert!(result.is_ok());
    let result = result.unwrap();
    if let Err(e) = result {
        match e.kind() {
            ErrorKind::Protocol(r) => {
                assert_eq!(r, vaux_mqtt::Reason::SessionTakeOver);
            }
            _ => panic!("Expected takeover error"),
        }
    } else {
        panic!("Expected takeover error");
    }
}
