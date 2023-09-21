use std::{io::Read, sync::Arc};

use clap::Parser;
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
    #[arg(short = 'c', long)]
    trusted_ca: Option<String>,
    #[arg(short, long)]
    clean_start: bool,
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

fn main() {
    let args = Args::parse();
    let mut root_store = rustls::RootCertStore::empty();
    if args.tls {
        if let Some(ca) = &args.trusted_ca {
            if !std::path::Path::new(ca).exists() {
                eprintln!("trusted CA file does not exist");
                return;
            }
            let cert = load_cert(ca).unwrap();
            root_store.add(&cert).unwrap();
        } else {
            eprintln!("trusted CA file required for TLS");
            return;
        }
    }
    let mut connection = vaux_client::MqttConnection::new();
    if args.tls {
        connection = connection.with_tls().with_trust_store(Arc::new(root_store))
    }
    connection = connection
        .with_host(&args.addr)
        .with_port(args.port)
        .connect()
        .unwrap();
    let client = vaux_client::MqttClient::new("vaux-subscriber-001", false, 10, false);
    subscribe(connection, client, args);
}

fn subscribe(connection: vaux_client::MqttConnection, mut client: MqttClient, args: Args) {
    client.start(connection, args.clean_start);
    let consumer = client.consumer();
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
    match producer.send(Packet::Subscribe(subscribe)) {
        Ok(_) => {
            loop {
                let iter = consumer.try_iter();
                for packet in iter {
                    if let Packet::Publish(mut p) = packet {
                        if p.properties().has_property(&PropertyType::PayloadFormat) {
                            if let Property::PayloadFormat(indicator) = p
                                .properties()
                                .get_property(&PropertyType::PayloadFormat)
                                .unwrap()
                            {
                                if *indicator == PayloadFormat::Utf8 {
                                    print!("Payload: ");
                                    println!(
                                        "{}",
                                        String::from_utf8(p.take_payload().unwrap()).unwrap()
                                    );
                                }
                            }
                        }
                        if args.auto_ack {
                            // check for QOS 1 or 2
                            match p.qos() {
                                QoSLevel::AtLeastOnce => {
                                    let mut ack = PubResp::new_puback();
                                    ack.packet_id = p.packet_id.unwrap();
                                    if let Err(e) = producer.send(Packet::PubAck(ack)) {
                                        eprintln!("{:?}", e);
                                    }
                                }
                                QoSLevel::ExactlyOnce => {
                                    let mut ack = PubResp::new_pubrec();
                                    ack.packet_id = p.packet_id.unwrap();
                                    if let Err(e) = producer.send(Packet::PubRec(ack)) {
                                        eprintln!("{:?}", e);
                                    }
                                }
                                _ => {}
                            }
                        } else {
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("{:?}", e);
        }
    }
}

fn load_cert(path: &str) -> Result<rustls::Certificate, std::io::Error> {
    let mut cert_buffer = Vec::new();
    let cert_file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(cert_file);
    reader.read_to_end(&mut cert_buffer)?;
    Ok(rustls::Certificate(cert_buffer))
}
