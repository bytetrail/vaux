use std::{net::SocketAddr, time::Duration};

use vaux_broker::{Broker, Config};
use vaux_client::{ClientBuilder, MqttConnection};
use vaux_mqtt::{Publish, QoSLevel};

const CONNECT_TIMEOUT: Duration = Duration::from_millis(5000);
const RECV_TIMEOUT: Duration = Duration::from_secs(3);

async fn start_broker(port: u16) -> Broker {
    let mut broker = Broker::new_with_config(Config {
        listen_addr: SocketAddr::from(([127, 0, 0, 1], port)),
        default_keep_alive: Duration::from_secs(30),
        max_keep_alive: Duration::from_secs(30),
        session_expiry: Duration::from_secs(60),
        ..Default::default()
    });
    broker.run().await.expect("failed to start broker");
    broker
}

struct TestClient {
    client: vaux_client::MqttClient,
    consumer: tokio::sync::mpsc::Receiver<vaux_mqtt::Packet>,
}

impl TestClient {
    async fn new(port: u16, client_id: &str) -> Self {
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
        .expect("failed to create client");

        client
            .try_start(CONNECT_TIMEOUT, true)
            .await
            .expect("failed to start client");

        let consumer = client
            .take_packet_consumer()
            .expect("failed to get consumer");

        Self { client, consumer }
    }

    async fn subscribe(&mut self, packet_id: u16, topics: &[&str], qos: QoSLevel) {
        self.client
            .subscribe(packet_id, topics, qos)
            .await
            .expect("failed to subscribe");

        let packet = tokio::time::timeout(RECV_TIMEOUT, self.consumer.recv())
            .await
            .expect("timed out waiting for SubAck")
            .expect("channel closed");
        assert!(matches!(packet, vaux_mqtt::Packet::SubAck(_)));
    }

    async fn publish(&mut self, topic: &str, payload: &[u8], qos: QoSLevel) {
        let publish = if qos == QoSLevel::AtMostOnce {
            Publish::new_with_payload(None, topic.to_string(), qos, payload.to_vec()).unwrap()
        } else {
            Publish::new_with_payload(Some(1), topic.to_string(), qos, payload.to_vec()).unwrap()
        };
        self.client
            .packet_producer()
            .send(vaux_mqtt::Packet::Publish(publish))
            .await
            .expect("failed to send publish");
    }

    async fn recv_publish(&mut self) -> Publish {
        let packet = tokio::time::timeout(RECV_TIMEOUT, self.consumer.recv())
            .await
            .expect("timed out waiting for message")
            .expect("channel closed");
        match packet {
            vaux_mqtt::Packet::Publish(p) => p,
            other => panic!("expected Publish, got {:?}", other),
        }
    }

    async fn expect_no_message(&mut self) {
        let result = tokio::time::timeout(Duration::from_millis(500), self.consumer.recv()).await;
        assert!(
            result.is_err(),
            "expected no message but received one"
        );
    }

    async fn stop(mut self) {
        self.client.stop().await.expect("failed to stop client");
    }
}

// ──────────────────────────────────────────────────────────────
// Basic exact-topic subscription
// ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn subscribe_exact_topic() {
    const PORT: u16 = 8400;
    let mut broker = start_broker(PORT).await;

    let mut subscriber = TestClient::new(PORT, "sub-exact").await;
    let mut publisher = TestClient::new(PORT, "pub-exact").await;

    subscriber
        .subscribe(1, &["sensor/temp"], QoSLevel::AtMostOnce)
        .await;

    publisher
        .publish("sensor/temp", b"22.5", QoSLevel::AtMostOnce)
        .await;

    let msg = subscriber.recv_publish().await;
    assert_eq!(msg.topic_name, "sensor/temp");
    assert_eq!(msg.payload, Some(bytes::Bytes::from_static(b"22.5")));

    // non-matching topic should not arrive
    publisher
        .publish("sensor/humidity", b"60", QoSLevel::AtMostOnce)
        .await;
    subscriber.expect_no_message().await;

    subscriber.stop().await;
    publisher.stop().await;
    broker.stop().await;
}

// ──────────────────────────────────────────────────────────────
// # wildcard subscription
// ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn subscribe_hash_wildcard() {
    const PORT: u16 = 8401;
    let mut broker = start_broker(PORT).await;

    let mut subscriber = TestClient::new(PORT, "sub-hash").await;
    let mut publisher = TestClient::new(PORT, "pub-hash").await;

    subscriber
        .subscribe(1, &["sensor/#"], QoSLevel::AtMostOnce)
        .await;

    publisher
        .publish("sensor/temp", b"22.5", QoSLevel::AtMostOnce)
        .await;
    let msg = subscriber.recv_publish().await;
    assert_eq!(msg.topic_name, "sensor/temp");

    publisher
        .publish("sensor/temp/room1", b"23.0", QoSLevel::AtMostOnce)
        .await;
    let msg = subscriber.recv_publish().await;
    assert_eq!(msg.topic_name, "sensor/temp/room1");

    // parent level matches '#' too
    publisher
        .publish("sensor", b"ping", QoSLevel::AtMostOnce)
        .await;
    let msg = subscriber.recv_publish().await;
    assert_eq!(msg.topic_name, "sensor");

    // non-matching prefix
    publisher
        .publish("other/topic", b"nope", QoSLevel::AtMostOnce)
        .await;
    subscriber.expect_no_message().await;

    subscriber.stop().await;
    publisher.stop().await;
    broker.stop().await;
}

// ──────────────────────────────────────────────────────────────
// + wildcard subscription
// ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn subscribe_plus_wildcard() {
    const PORT: u16 = 8402;
    let mut broker = start_broker(PORT).await;

    let mut subscriber = TestClient::new(PORT, "sub-plus").await;
    let mut publisher = TestClient::new(PORT, "pub-plus").await;

    subscriber
        .subscribe(1, &["sensor/+/reading"], QoSLevel::AtMostOnce)
        .await;

    publisher
        .publish("sensor/temp/reading", b"22.5", QoSLevel::AtMostOnce)
        .await;
    let msg = subscriber.recv_publish().await;
    assert_eq!(msg.topic_name, "sensor/temp/reading");

    publisher
        .publish("sensor/humidity/reading", b"60", QoSLevel::AtMostOnce)
        .await;
    let msg = subscriber.recv_publish().await;
    assert_eq!(msg.topic_name, "sensor/humidity/reading");

    // '+' does not match multiple levels
    publisher
        .publish("sensor/a/b/reading", b"nope", QoSLevel::AtMostOnce)
        .await;
    subscriber.expect_no_message().await;

    // wrong final segment
    publisher
        .publish("sensor/temp/value", b"nope", QoSLevel::AtMostOnce)
        .await;
    subscriber.expect_no_message().await;

    subscriber.stop().await;
    publisher.stop().await;
    broker.stop().await;
}

// ──────────────────────────────────────────────────────────────
// Multiple subscribers on the same topic (non-shared)
// ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn multiple_subscribers_same_topic() {
    const PORT: u16 = 8403;
    let mut broker = start_broker(PORT).await;

    let mut sub_a = TestClient::new(PORT, "sub-multi-a").await;
    let mut sub_b = TestClient::new(PORT, "sub-multi-b").await;
    let mut publisher = TestClient::new(PORT, "pub-multi").await;

    sub_a
        .subscribe(1, &["events/alert"], QoSLevel::AtMostOnce)
        .await;
    sub_b
        .subscribe(1, &["events/alert"], QoSLevel::AtMostOnce)
        .await;

    publisher
        .publish("events/alert", b"fire", QoSLevel::AtMostOnce)
        .await;

    let msg_a = sub_a.recv_publish().await;
    assert_eq!(msg_a.topic_name, "events/alert");
    assert_eq!(msg_a.payload, Some(bytes::Bytes::from_static(b"fire")));

    let msg_b = sub_b.recv_publish().await;
    assert_eq!(msg_b.topic_name, "events/alert");
    assert_eq!(msg_b.payload, Some(bytes::Bytes::from_static(b"fire")));

    sub_a.stop().await;
    sub_b.stop().await;
    publisher.stop().await;
    broker.stop().await;
}

// ──────────────────────────────────────────────────────────────
// Multiple subscribers with overlapping wildcards
// ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn multiple_subscribers_overlapping_wildcards() {
    const PORT: u16 = 8404;
    let mut broker = start_broker(PORT).await;

    let mut sub_exact = TestClient::new(PORT, "sub-overlap-exact").await;
    let mut sub_hash = TestClient::new(PORT, "sub-overlap-hash").await;
    let mut sub_plus = TestClient::new(PORT, "sub-overlap-plus").await;
    let mut publisher = TestClient::new(PORT, "pub-overlap").await;

    sub_exact
        .subscribe(1, &["sensor/temp"], QoSLevel::AtMostOnce)
        .await;
    sub_hash
        .subscribe(1, &["sensor/#"], QoSLevel::AtMostOnce)
        .await;
    sub_plus
        .subscribe(1, &["sensor/+"], QoSLevel::AtMostOnce)
        .await;

    publisher
        .publish("sensor/temp", b"data", QoSLevel::AtMostOnce)
        .await;

    // all three should receive the message
    let msg = sub_exact.recv_publish().await;
    assert_eq!(msg.topic_name, "sensor/temp");

    let msg = sub_hash.recv_publish().await;
    assert_eq!(msg.topic_name, "sensor/temp");

    let msg = sub_plus.recv_publish().await;
    assert_eq!(msg.topic_name, "sensor/temp");

    sub_exact.stop().await;
    sub_hash.stop().await;
    sub_plus.stop().await;
    publisher.stop().await;
    broker.stop().await;
}
