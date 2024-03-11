use std::{io::Read, sync::Arc, time::Duration};

use clap::{error::ErrorKind, Parser};
use vaux_mqtt::{property::Property, publish::Publish, Packet, QoSLevel};

#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, default_value = "0", value_parser = QoSLevelParser)]
    qos: QoSLevel,
    #[arg(short, long, default_value = "hello-vaux")]
    topic: String,
    #[arg(short, long, default_value = "localhost")]
    addr: String,
    #[arg(short = 's', long, requires = "trusted_ca")]
    tls: bool,
    #[arg(short, long, default_value = "1883")]
    pub port: u16,
    #[arg(short = 'c', long)]
    trusted_ca: Option<String>,
    #[arg(short = 'w', long, requires = "password")]
    username: Option<String>,
    #[arg(short = 'u', long, requires = "username")]
    password: Option<String>,
    message: String,
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
            _ => Err(clap::Error::new(ErrorKind::InvalidValue)),
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
    let mut client = vaux_client::MqttClient::new("vaux-publisher-001", false, 10, false);
    publish(&mut client, connection, args.clone());
}

fn publish(
    client: &mut vaux_client::MqttClient,
    connection: vaux_client::MqttConnection,
    args: Args,
) {
    let handle: Option<std::thread::JoinHandle<_>> =
        match client.try_start(Duration::from_millis(5000), connection, true) {
            Ok(h) => Some(h),
            Err(e) => {
                eprintln!("unable to start client: {:?}", e);
                return;
            }
        };
    let producer = client.producer();
    let consumer = client.consumer();

    let mut publish = Publish::default();
    publish
        .properties_mut()
        .set_property(Property::PayloadFormat(
            vaux_mqtt::property::PayloadFormat::Utf8,
        ));
    publish
        .properties_mut()
        .set_property(Property::MessageExpiry(1000));

    publish.topic_name = Some(args.topic);
    publish.set_payload(Vec::from(args.message.as_bytes()));
    publish.set_qos(args.qos);
    publish.packet_id = Some(101);
    if producer
        .send(vaux_mqtt::Packet::Publish(publish.clone()))
        .is_err()
    {
        eprintln!("unable to send packet to broker");
    } else {
        println!("sent message");
    }

    let mut packet = consumer.recv_timeout(Duration::from_millis(1000));
    let mut ack_recv = false;
    while !ack_recv {
        if packet.is_err() {
            println!("waiting for ack");
        } else if let Ok(Packet::PubAck(ack)) = packet {
            println!("received ack: {:?}", ack);
            ack_recv = true;
        }
        packet = consumer.recv_timeout(Duration::from_millis(1000));
    }

    match client.stop() {
        Ok(_) => (),
        Err(e) => eprintln!("unable to stop client: {:?}", e),
    }
    if let Some(h) = handle {
        match h.join() {
            Ok(r) => match r {
                Ok(_) => (),
                Err(e) => eprintln!("client thread failed: {:?}", e),
            },
            Err(e) => eprintln!("unable to join client thread: {:?}", e),
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
