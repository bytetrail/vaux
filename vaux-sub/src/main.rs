use clap::Parser;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::CertificateDer;
use std::{io::Read, sync::Arc};
use vaux_client::MqttClient;
use vaux_mqtt::{
    property::{PayloadFormat, Property},
    Packet, PropertyType, PubResp, QoSLevel, Subscribe, Subscription,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    auto_ack: bool,
    #[arg(short, long, default_value = "0", value_parser = QoSLevelParser)]
    qos: QoSLevel,
    #[arg(short = 'b', long, default_value = "localhost")]
    addr: String,
    #[arg(short = 'p', long, default_value = "1883")]
    port: u16,
    #[arg(short = 's', long, requires = "trusted_ca")]
    tls: bool,
    #[arg(short = 't', long)]
    trusted_ca: Option<String>,
    #[arg(short, long)]
    clean_start: bool,
    #[arg(short = 'w', long, requires = "password")]
    username: Option<String>,
    #[arg(short = 'u', long, requires = "username")]
    password: Option<String>,
}

#[derive(Clone, Debug)]
pub struct QoSLevelParser;

impl clap::builder::TypedValueParser for QoSLevelParser {
    type Value = QoSLevel;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        match value.to_os_string().into_string().unwrap().as_str() {
            "0" => Ok(QoSLevel::AtMostOnce),
            "1" => Ok(QoSLevel::AtLeastOnce),
            "2" => Ok(QoSLevel::ExactlyOnce),
            _ => Err(clap::Error::new(clap::error::ErrorKind::InvalidValue)),
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    let mut root_store = rustls::RootCertStore::empty();
    if args.tls {
        if let Some(ca) = &args.trusted_ca {
            if !std::path::Path::new(ca).exists() {
                eprintln!("trusted CA file does not exist");
                return;
            }
            let cert = load_cert(ca).unwrap();
            root_store.add(cert).unwrap();
        } else {
            eprintln!("trusted CA file required for TLS");
            return;
        }
    }
    let mut connection = vaux_client::MqttConnection::new();
    if args.tls {
        connection = connection.with_tls().with_trust_store(Arc::new(root_store))
    }
    connection = connection.with_host(&args.addr).with_port(args.port);
    let client = vaux_client::MqttClient::new_with_connection(
        connection,
        "vaux-subscriber-001",
        false,
        10,
        false,
    );
    println!("subscribing to {}", args.addr);
    subscribe(client, args).await;
}

async fn subscribe(mut client: MqttClient, args: Args) {
    client.set_keep_alive(10);
    let handle = client.start(args.clean_start).await;
    println!("started");
    let mut consumer = client.take_consumer().unwrap();
    let producer = client.producer();
    let filter = vec![
        // inbound device ops messages for this shadow on this site
        Subscription {
            filter: "hello-vaux".to_string(),
            qos: args.qos,
            no_local: false,
            retain_as: false,
            handling: vaux_mqtt::subscribe::RetainHandling::None,
        },
    ];
    let subscribe = Subscribe::new(1, filter);
    match producer.send(Packet::Subscribe(subscribe)).await {
        Ok(_) => {
            loop {
                tokio::task::yield_now().await;
                let iter = consumer.try_recv();
                if let Ok(packet) = iter {
                    if let Packet::Publish(mut p) = packet {
                        if p.properties().has_property(&PropertyType::PayloadFormat) {
                            if let Property::PayloadFormat(indicator) = p
                                .properties()
                                .get_property(&PropertyType::PayloadFormat)
                                .unwrap()
                            {
                                match indicator {
                                    PayloadFormat::Utf8 => {
                                        let message =
                                            String::from_utf8(p.take_payload().unwrap()).unwrap();
                                        println!("{}", message);
                                    }
                                    PayloadFormat::Bin => {
                                        println!("received a binary payload");
                                    }
                                }
                            }
                        }
                        if args.auto_ack {
                            // check for QOS 1 or 2
                            match p.qos() {
                                QoSLevel::AtLeastOnce => {
                                    let mut ack = PubResp::new_puback();
                                    ack.packet_id = p.packet_id.unwrap();
                                    if let Err(e) = producer.send(Packet::PubAck(ack)).await {
                                        eprintln!("{:?}", e);
                                    }
                                }
                                QoSLevel::ExactlyOnce => {
                                    let mut ack = PubResp::new_pubrec();
                                    ack.packet_id = p.packet_id.unwrap();
                                    if let Err(e) = producer.send(Packet::PubRec(ack)).await {
                                        eprintln!("{:?}", e);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("{:?}", e);
        }
    }
    let _ = handle.await;
}

fn load_cert(path: &str) -> Result<CertificateDer, std::io::Error> {
    let mut cert_buffer = Vec::new();
    let cert_file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(cert_file);
    reader.read_to_end(&mut cert_buffer)?;
    Ok(CertificateDer::from_pem_slice(&cert_buffer).unwrap())
}
