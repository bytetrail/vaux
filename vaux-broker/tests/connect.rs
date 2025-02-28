use std::time::Duration;

use vaux_client::MqttConnection;

#[tokio::test]
pub async fn basic_connect() {
    let listen_addr = "127.0.0.1:8383";
    let mut broker = vaux_broker::Broker::new(listen_addr.parse().unwrap());
    let result = broker.run().await;
    assert!(result.is_ok());

    let client = vaux_client::ClientBuilder::new(
        MqttConnection::new().with_host("127.0.0.1").with_port(8383),
    )
    .with_client_id("client_id")
    .with_auto_ack(true)
    .with_keep_alive(Duration::from_secs(30))
    .build();
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
    let listen_addr = "127.0.0.1:8384";
    let mut broker = vaux_broker::Broker::new(listen_addr.parse().unwrap());
    let result = broker.run().await;
    assert!(result.is_ok());

    let client_one = vaux_client::ClientBuilder::new(
        MqttConnection::new().with_host("127.0.0.1").with_port(8384),
    )
    .with_client_id("takeover_client_id")
    .with_auto_ack(true)
    .with_session_expiry(5555)
    .build();
    if let Ok(mut client) = client_one {
        let result = client.try_start(Duration::from_millis(5000), true).await;
        assert!(result.is_ok());
    } else {
        panic!("Failed to create client");
    }

    let client_two = vaux_client::ClientBuilder::new(
        MqttConnection::new().with_host("127.0.0.1").with_port(8384),
    )
    .with_client_id("takeover_client_id")
    .with_auto_ack(true)
    .with_session_expiry(6666)
    .build();
    if let Ok(mut client) = client_two {
        let result = client.try_start(Duration::from_millis(5000), true).await;
        println!("result: {:?}", result);
        assert!(result.is_ok());
    } else {
        panic!("Failed to create client");
    }
    broker.stop().await;
}
