use std::{net::SocketAddr, sync::Arc, time::Duration};

use vaux_broker::{Broker, Config};
use vaux_client::{ClientBuilder, MqttConnection};
use vaux_mqtt::QoSLevel;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(5000);
const RECV_TIMEOUT: Duration = Duration::from_secs(3);

fn generate_tls_config() -> (tokio_rustls::TlsAcceptor, Arc<rustls::RootCertStore>) {
    use rcgen::{CertificateParams, CertifiedIssuer, IsCa, BasicConstraints, KeyPair};

    // generate CA
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca = CertifiedIssuer::self_signed(ca_params, ca_key).unwrap();
    let ca_cert_der = rustls::pki_types::CertificateDer::from(ca.as_ref().der().to_vec());

    // generate server cert signed by CA
    let server_key = KeyPair::generate().unwrap();
    let mut server_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    server_params.subject_alt_names = vec![
        rcgen::SanType::DnsName("localhost".try_into().unwrap()),
        rcgen::SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        rcgen::SanType::IpAddress(std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)),
    ];
    let server_cert = server_params.signed_by(&server_key, &ca).unwrap();
    let server_cert_der = rustls::pki_types::CertificateDer::from(server_cert.der().to_vec());
    let server_key_der =
        rustls::pki_types::PrivateKeyDer::try_from(server_key.serialize_der()).unwrap();

    // build server TlsAcceptor
    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![server_cert_der, ca_cert_der.clone()], server_key_der)
        .expect("invalid server config");
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));

    // build client trust store
    let mut root_store = rustls::RootCertStore::empty();
    root_store.add(ca_cert_der).expect("failed to add CA cert");

    (acceptor, Arc::new(root_store))
}

#[tokio::test]
async fn tls_connect_and_publish() {
    const PORT: u16 = 8410;

    let (acceptor, trust_store) = generate_tls_config();

    let mut broker = Broker::new_with_config(Config {
        listen_addr: SocketAddr::from(([127, 0, 0, 1], PORT)),
        tls_acceptor: Some(acceptor),
        ..Default::default()
    });
    broker.run().await.expect("failed to start broker");

    // subscriber over TLS
    let mut sub_client = ClientBuilder::new(
        MqttConnection::new()
            .with_host("127.0.0.1")
            .with_port(PORT)
            .with_tls()
            .with_trust_store(trust_store.clone()),
    )
    .with_client_id("tls-sub")
    .with_auto_ack(true)
    .with_keep_alive(Duration::from_secs(60))
    .build()
    .await
    .expect("failed to create subscriber");

    sub_client
        .try_start(CONNECT_TIMEOUT, true)
        .await
        .expect("failed to start subscriber");

    let mut consumer = sub_client
        .take_packet_consumer()
        .expect("failed to get consumer");

    sub_client
        .subscribe(1, &["tls/test"], QoSLevel::AtMostOnce)
        .await
        .expect("failed to subscribe");

    let suback = tokio::time::timeout(RECV_TIMEOUT, consumer.recv())
        .await
        .expect("timed out waiting for SubAck")
        .expect("channel closed");
    assert!(matches!(suback, vaux_mqtt::Packet::SubAck(_)));

    // publisher over TLS
    let mut pub_client = ClientBuilder::new(
        MqttConnection::new()
            .with_host("127.0.0.1")
            .with_port(PORT)
            .with_tls()
            .with_trust_store(trust_store),
    )
    .with_client_id("tls-pub")
    .with_auto_ack(true)
    .with_keep_alive(Duration::from_secs(60))
    .build()
    .await
    .expect("failed to create publisher");

    pub_client
        .try_start(CONNECT_TIMEOUT, true)
        .await
        .expect("failed to start publisher");

    let publish = vaux_mqtt::Publish::new_with_payload(
        None,
        "tls/test".to_string(),
        QoSLevel::AtMostOnce,
        b"encrypted hello".to_vec(),
    )
    .unwrap();

    pub_client
        .packet_producer()
        .send(vaux_mqtt::Packet::Publish(publish))
        .await
        .expect("failed to send publish");

    let msg = tokio::time::timeout(RECV_TIMEOUT, consumer.recv())
        .await
        .expect("timed out waiting for message")
        .expect("channel closed");

    match msg {
        vaux_mqtt::Packet::Publish(p) => {
            assert_eq!(p.topic_name, "tls/test");
            assert_eq!(p.payload, Some(bytes::Bytes::from_static(b"encrypted hello")));
        }
        other => panic!("expected Publish, got {:?}", other),
    }

    sub_client.stop().await.expect("failed to stop subscriber");
    pub_client.stop().await.expect("failed to stop publisher");
    broker.stop().await;
}
