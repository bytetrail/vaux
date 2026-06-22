use std::{net::SocketAddr, time::Duration};

use vaux_broker::{Broker, Config};
use vaux_client::{ClientBuilder, MqttConnection};
use vaux_mqtt::{QoSLevel, WillMessage};

const CONNECT_TIMEOUT: Duration = Duration::from_millis(5000);

async fn start_broker(port: u16, keep_alive_secs: u16) -> Broker {
    let mut broker = Broker::new_with_config(Config {
        listen_addr: SocketAddr::from(([127, 0, 0, 1], port)),
        default_keep_alive: Duration::from_secs(keep_alive_secs as u64),
        max_keep_alive: Duration::from_secs(keep_alive_secs as u64),
        session_expiry: Duration::from_secs(60),
        ..Default::default()
    });
    broker.run().await.expect("failed to start broker");
    broker
}

async fn connect_subscriber(port: u16, client_id: &str, topic: &str) -> (vaux_client::MqttClient, tokio::sync::mpsc::Receiver<vaux_mqtt::Packet>) {
    let mut client = ClientBuilder::new(
        MqttConnection::new()
            .with_host("127.0.0.1")
            .with_port(port),
    )
    .with_client_id(client_id)
    .with_auto_ack(true)
    .with_keep_alive(Duration::from_secs(60))
    .build()
    .await
    .expect("failed to create subscriber");

    client
        .try_start(CONNECT_TIMEOUT, true)
        .await
        .expect("failed to start subscriber");

    let mut consumer = client.take_packet_consumer().expect("failed to get consumer");
    client
        .subscribe(1, &[topic], QoSLevel::AtLeastOnce)
        .await
        .expect("failed to subscribe");

    // consume the SubAck
    let packet = tokio::time::timeout(Duration::from_secs(2), consumer.recv())
        .await
        .expect("timed out waiting for SubAck")
        .expect("channel closed");
    assert!(matches!(packet, vaux_mqtt::Packet::SubAck(_)));

    (client, consumer)
}

async fn connect_publisher_with_will(port: u16, client_id: &str, will: WillMessage) -> (vaux_client::MqttClient, tokio::task::JoinHandle<impl std::any::Any>) {
    let mut client = ClientBuilder::new(
        MqttConnection::new()
            .with_host("127.0.0.1")
            .with_port(port),
    )
    .with_client_id(client_id)
    .with_auto_ack(true)
    .with_keep_alive(Duration::from_secs(60))
    .with_will_message(will)
    .build()
    .await
    .expect("failed to create publisher");

    let handle = client
        .try_start(CONNECT_TIMEOUT, true)
        .await
        .expect("failed to start publisher");

    (client, handle)
}

/// Test that a will message is delivered when a client drops its network
/// connection without sending DISCONNECT.
#[tokio::test]
async fn will_on_network_close() {
    const PORT: u16 = 8390;
    const WILL_TOPIC: &str = "test/will/close";
    const WILL_PAYLOAD: &[u8] = b"client gone";

    let mut broker = start_broker(PORT, 30).await;

    let (mut subscriber, mut consumer) = connect_subscriber(PORT, "will-sub-close", WILL_TOPIC).await;

    let will = WillMessage::new(
        WILL_TOPIC.to_string(),
        WILL_PAYLOAD,
        QoSLevel::AtLeastOnce,
        false,
    );
    let (_publisher, handle) = connect_publisher_with_will(PORT, "will-pub-close", will).await;

    // abort the client task — forces TCP stream drop without sending DISCONNECT
    handle.abort();
    let _ = handle.await;

    let packet = tokio::time::timeout(Duration::from_secs(5), consumer.recv())
        .await
        .expect("timed out waiting for will message")
        .expect("channel closed");

    match packet {
        vaux_mqtt::Packet::Publish(publish) => {
            assert_eq!(publish.topic_name, WILL_TOPIC);
            assert_eq!(publish.payload, Some(bytes::Bytes::from_static(WILL_PAYLOAD)));
        }
        other => panic!("expected Publish, got {:?}", other),
    }

    subscriber.stop().await.expect("failed to stop subscriber");
    broker.stop().await;
}

/// Test that a will message is delivered when the broker's keep-alive timer
/// expires because the client stopped communicating.
#[tokio::test]
async fn will_on_keep_alive_expiry() {
    const PORT: u16 = 8391;
    const WILL_TOPIC: &str = "test/will/keepalive";
    const WILL_PAYLOAD: &[u8] = b"timed out";
    // broker enforces 2-second keep alive; with 1.5x factor the timer fires at ~3s
    const BROKER_KEEP_ALIVE_SECS: u16 = 2;

    let mut broker = start_broker(PORT, BROKER_KEEP_ALIVE_SECS).await;

    let (mut subscriber, mut consumer) = connect_subscriber(PORT, "will-sub-ka", WILL_TOPIC).await;

    let will = WillMessage::new(
        WILL_TOPIC.to_string(),
        WILL_PAYLOAD,
        QoSLevel::AtMostOnce,
        false,
    );

    // connect with keep-alive pings disabled — the broker's keep-alive timer
    // will expire because the client never sends any traffic
    let mut silent_client = ClientBuilder::new(
        MqttConnection::new()
            .with_host("127.0.0.1")
            .with_port(PORT),
    )
    .with_client_id("will-pub-ka")
    .with_auto_ack(true)
    .with_keep_alive(Duration::from_secs(60))
    .with_keep_alive_enabled(false)
    .with_will_message(will)
    .build()
    .await
    .expect("failed to create silent client");

    let _handle = silent_client
        .try_start(CONNECT_TIMEOUT, true)
        .await
        .expect("failed to start silent client");

    // wait for the broker's keep-alive timer to fire (~3s with 1.5x factor)
    let packet = tokio::time::timeout(Duration::from_secs(10), consumer.recv())
        .await
        .expect("timed out waiting for will message")
        .expect("channel closed");

    match packet {
        vaux_mqtt::Packet::Publish(publish) => {
            assert_eq!(publish.topic_name, WILL_TOPIC);
            assert_eq!(publish.payload, Some(bytes::Bytes::from_static(WILL_PAYLOAD)));
        }
        other => panic!("expected Publish, got {:?}", other),
    }

    subscriber.stop().await.expect("failed to stop subscriber");
    broker.stop().await;
}

/// Test that a will message is NOT delivered when a client disconnects
/// normally with reason code 0x00.
#[tokio::test]
async fn no_will_on_normal_disconnect() {
    const PORT: u16 = 8392;
    const WILL_TOPIC: &str = "test/will/normal";
    const WILL_PAYLOAD: &[u8] = b"should not arrive";

    let mut broker = start_broker(PORT, 30).await;

    let (mut subscriber, mut consumer) = connect_subscriber(PORT, "will-sub-normal", WILL_TOPIC).await;

    let will = WillMessage::new(
        WILL_TOPIC.to_string(),
        WILL_PAYLOAD,
        QoSLevel::AtMostOnce,
        false,
    );
    let (mut publisher, _handle) = connect_publisher_with_will(PORT, "will-pub-normal", will).await;

    // normal disconnect — will should be cleared, not published
    publisher.stop().await.expect("failed to stop publisher");

    // wait a bit and verify no will message arrives
    let result = tokio::time::timeout(Duration::from_secs(2), consumer.recv()).await;
    assert!(result.is_err(), "expected timeout — will should not have been delivered");

    subscriber.stop().await.expect("failed to stop subscriber");
    broker.stop().await;
}
