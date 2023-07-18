use std::net::Ipv4Addr;

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
    #[arg(short = 'b', long, default_value = "127.0.0.1")]
    addr: String,
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

    let addr: Ipv4Addr = "127.0.0.1".parse().expect("unable to create addr");
    let mut client = MqttClient::new(
        std::net::IpAddr::V4(addr),
        1883,
        "vaux-subscriber-001",
        false,
        10,
        false,
    );
    match client.connect(args.clean_start) {
        Ok(c) => {
            println!("Connected: {:?}", c);
            client.start();
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
                        let mut iter = consumer.try_iter();
                        while let Some(packet) = iter.next() {
                            match packet {
                                Packet::Publish(mut p) => {
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
                                                    String::from_utf8(p.take_payload().unwrap())
                                                        .unwrap()
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

                                _ => {}
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {:?}", e);
        }
    }
}
