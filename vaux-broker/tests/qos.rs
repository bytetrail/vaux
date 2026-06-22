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

    async fn stop(mut self) {
        self.client.stop().await.expect("failed to stop client");
    }
}

/// Publisher sends QoS 1 → subscriber receives the message with a valid
/// packet ID → subscriber's auto_ack sends PubAck back to broker.
#[tokio::test]
async fn qos1_publish_to_subscriber() {
    const PORT: u16 = 8420;
    let mut broker = start_broker(PORT).await;

    let mut subscriber = TestClient::new(PORT, "qos1-sub").await;
    let mut publisher = TestClient::new(PORT, "qos1-pub").await;

    subscriber
        .subscribe(1, &["qos/test"], QoSLevel::AtLeastOnce)
        .await;

    let publish = Publish::new_with_payload(
        Some(10),
        "qos/test".to_string(),
        QoSLevel::AtLeastOnce,
        b"qos1 payload".to_vec(),
    )
    .unwrap();

    publisher
        .client
        .packet_producer()
        .send(vaux_mqtt::Packet::Publish(publish))
        .await
        .expect("failed to send publish");

    let msg = subscriber.recv_publish().await;
    assert_eq!(msg.topic_name, "qos/test");
    assert_eq!(msg.payload, Some(bytes::Bytes::from_static(b"qos1 payload")));
    assert_eq!(msg.qos(), QoSLevel::AtLeastOnce);
    assert!(msg.packet_id().is_some(), "QoS 1 message should have a packet ID");
    assert_ne!(msg.packet_id().unwrap(), 0);

    subscriber.stop().await;
    publisher.stop().await;
    broker.stop().await;
}

/// Multiple QoS 1 messages should get unique packet IDs from the broker.
#[tokio::test]
async fn qos1_unique_packet_ids() {
    const PORT: u16 = 8421;
    let mut broker = start_broker(PORT).await;

    let mut subscriber = TestClient::new(PORT, "qos1-id-sub").await;
    let mut publisher = TestClient::new(PORT, "qos1-id-pub").await;

    subscriber
        .subscribe(1, &["id/test"], QoSLevel::AtLeastOnce)
        .await;

    for i in 1..=3u16 {
        let publish = Publish::new_with_payload(
            Some(i),
            "id/test".to_string(),
            QoSLevel::AtLeastOnce,
            format!("msg-{i}").into_bytes(),
        )
        .unwrap();

        publisher
            .client
            .packet_producer()
            .send(vaux_mqtt::Packet::Publish(publish))
            .await
            .expect("failed to send publish");
    }

    let mut packet_ids = Vec::new();
    for _ in 0..3 {
        let msg = subscriber.recv_publish().await;
        assert_eq!(msg.qos(), QoSLevel::AtLeastOnce);
        let pid = msg.packet_id().expect("missing packet ID");
        packet_ids.push(pid);
    }

    // all packet IDs should be unique
    packet_ids.sort();
    packet_ids.dedup();
    assert_eq!(packet_ids.len(), 3, "expected 3 unique packet IDs, got {:?}", packet_ids);

    subscriber.stop().await;
    publisher.stop().await;
    broker.stop().await;
}

/// QoS 0 messages should be delivered without packet IDs.
#[tokio::test]
async fn qos0_no_packet_id() {
    const PORT: u16 = 8422;
    let mut broker = start_broker(PORT).await;

    let mut subscriber = TestClient::new(PORT, "qos0-sub").await;
    let mut publisher = TestClient::new(PORT, "qos0-pub").await;

    subscriber
        .subscribe(1, &["qos0/test"], QoSLevel::AtMostOnce)
        .await;

    let publish = Publish::new_with_payload(
        None,
        "qos0/test".to_string(),
        QoSLevel::AtMostOnce,
        b"fire and forget".to_vec(),
    )
    .unwrap();

    publisher
        .client
        .packet_producer()
        .send(vaux_mqtt::Packet::Publish(publish))
        .await
        .expect("failed to send publish");

    let msg = subscriber.recv_publish().await;
    assert_eq!(msg.topic_name, "qos0/test");
    assert_eq!(msg.qos(), QoSLevel::AtMostOnce);
    assert_eq!(msg.packet_id(), None);

    subscriber.stop().await;
    publisher.stop().await;
    broker.stop().await;
}

/// QoS downgrade: publisher sends QoS 1 but subscriber subscribed at QoS 0.
/// Subscriber should receive at QoS 0 (no packet ID).
#[tokio::test]
async fn qos_downgrade() {
    const PORT: u16 = 8423;
    let mut broker = start_broker(PORT).await;

    let mut subscriber = TestClient::new(PORT, "qos-down-sub").await;
    let mut publisher = TestClient::new(PORT, "qos-down-pub").await;

    // subscribe at QoS 0
    subscriber
        .subscribe(1, &["down/test"], QoSLevel::AtMostOnce)
        .await;

    // publish at QoS 1
    let publish = Publish::new_with_payload(
        Some(1),
        "down/test".to_string(),
        QoSLevel::AtLeastOnce,
        b"downgraded".to_vec(),
    )
    .unwrap();

    publisher
        .client
        .packet_producer()
        .send(vaux_mqtt::Packet::Publish(publish))
        .await
        .expect("failed to send publish");

    let msg = subscriber.recv_publish().await;
    assert_eq!(msg.topic_name, "down/test");
    assert_eq!(msg.qos(), QoSLevel::AtMostOnce, "should be downgraded to QoS 0");
    assert_eq!(msg.packet_id(), None, "QoS 0 should have no packet ID");

    subscriber.stop().await;
    publisher.stop().await;
    broker.stop().await;
}
