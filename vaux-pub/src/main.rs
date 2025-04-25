use std::{io::Read, sync::Arc, time::Duration};

use clap::{error::ErrorKind, Parser};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::CertificateDer;
use tokio::{select, task::JoinHandle};
use vaux_client::PacketChannel;
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
    #[arg(short = 'f', long, group = "payload")]
    message_file: Option<String>,
    #[arg(short = 'm', long, group = "payload")]
    message: Option<String>,
    #[arg(short = 'i', long)]
    iterations: Option<u32>,
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

#[tokio::main]
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
    let connection = connection.with_host(&args.addr).with_port(args.port);
    let mut puback_channel = PacketChannel::new_with_size(10);

    let mut client = vaux_client::ClientBuilder::new(connection)
        .with_auto_ack(true)
        .with_auto_packet_id(true)
        .with_receive_max(10)
        .with_session_expiry(1000)
        .with_keep_alive(Duration::from_secs(30))
        .with_max_connect_wait(Duration::from_secs(5))
        .with_filtered_consumer(vaux_mqtt::PacketType::PubAck, puback_channel.sender())
        .with_filtered_consumer(vaux_mqtt::PacketType::PubComp, puback_channel.sender())
        .with_filtered_consumer(vaux_mqtt::PacketType::PubRec, puback_channel.sender())
        .build()
        .await
        .unwrap();

    let mut packet_in = client.take_packet_consumer().unwrap();
    let producer = client.packet_producer();

    publish(
        &mut client,
        producer,
        &mut packet_in,
        &mut puback_channel.take_receiver().unwrap(),
        args.clone(),
    )
    .await;
}

async fn publish(
    client: &mut vaux_client::MqttClient,
    packet_out: tokio::sync::mpsc::Sender<Packet>,
    packet_in: &mut tokio::sync::mpsc::Receiver<Packet>,
    puback_recv: &mut tokio::sync::mpsc::Receiver<Packet>,
    args: Args,
) {
    let handle: Option<JoinHandle<_>> =
        match client.try_start(Duration::from_millis(5000), true).await {
            Ok(h) => Some(h),
            Err(e) => {
                eprintln!("unable to start client: {:?}", e);
                return;
            }
        };
    let iterations = args.iterations.unwrap_or(1);
    let topic = args.topic.clone();
    let arg_message = if let Some(m) = args.message {
        m
    } else if let Some(f) = args.message_file {
        let mut file = std::fs::File::open(f).unwrap();
        let mut buffer = String::new();
        file.read_to_string(&mut buffer).unwrap();
        buffer
    } else {
        "hello world".to_string()
    };
    let start = std::time::Instant::now();
    for i in 0..iterations {
        let mut publish = Publish::default();
        publish
            .properties_mut()
            .set_property(Property::PayloadFormat(
                vaux_mqtt::property::PayloadFormat::Utf8,
            ));
        publish
            .properties_mut()
            .set_property(Property::MessageExpiry(1000));

        let message = arg_message.clone();
        publish.topic_name = Some(topic.clone());
        publish.set_payload(Vec::from(message.as_bytes()));
        publish.set_qos(args.qos);
        publish.packet_id = Some((i + 1) as u16);
        if packet_out
            .send(vaux_mqtt::Packet::Publish(publish.clone()))
            .await
            .is_err()
        {
            eprintln!("unable to send packet to broker");
        }
        let mut pub_ack_recv = false;
        let mut pub_comp_recv = false;
        let mut pub_rec_recv = false;
        loop {
            select! {
                _ = tokio::time::sleep(Duration::from_millis(1000)) => {},
                puback = puback_recv.recv() => {
                    if let Some(packet) = puback {
                        if let Packet::PubAck(p) = packet {
                            match args.qos {
                                QoSLevel::AtLeastOnce => {
                                    println!("received puback: {:?}", p);
                                    pub_ack_recv = true;
                                }
                                _ => eprintln!("unexpected packet type: {:?}", p),
                            }
                        } else if let Packet::PubComp(p) = packet {
                            match args.qos {
                                QoSLevel::ExactlyOnce => {
                                    println!("received pubcomp: {:?}", p);
                                    pub_comp_recv = true;
                                }
                                _ => eprintln!("unexpected packet type: {:?}", p),
                            }
                        } else if let Packet::PubRec(p) = packet {
                            match args.qos {
                                QoSLevel::ExactlyOnce => {
                                    println!("received pubrec: {:?}", p);
                                    pub_rec_recv = true;
                                }
                                _ => eprintln!("unexpected packet type: {:?}", p),
                            }
                        } else {
                            eprintln!("unexpected packet type: {:?}", packet);
                        }
                    } else {
                        eprintln!("unable to receive puback");
                    }
                }
                packet = packet_in.recv() => {
                    println!("received packet: {:?}", packet);
                }
            }
            if match args.qos {
                QoSLevel::AtMostOnce => true,
                QoSLevel::AtLeastOnce => pub_ack_recv,
                QoSLevel::ExactlyOnce => pub_comp_recv && pub_rec_recv,
            } {
                break;
            }
        }
    }

    println!("elapsed time: {:?}", start.elapsed());
    match client.stop().await {
        Ok(_) => (),
        Err(e) => eprintln!("unable to stop client: {:?}", e),
    }
    if let Some(h) = handle {
        println!("waiting for client thread to finish");
        match h.await {
            Ok(r) => match r {
                Ok(_) => (),
                Err(e) => eprintln!("client thread failed: {:?}", e),
            },
            Err(e) => eprintln!("unable to join client thread: {:?}", e),
        }
    }
}

fn load_cert(path: &str) -> Result<CertificateDer, std::io::Error> {
    let mut cert_buffer = Vec::new();
    let cert_file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(cert_file);
    reader.read_to_end(&mut cert_buffer)?;
    Ok(CertificateDer::from_pem_slice(&cert_buffer).unwrap())
}
