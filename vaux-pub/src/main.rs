use std::net::{IpAddr, Ipv4Addr};

use clap::{error::ErrorKind, Parser};
use vaux_mqtt::{property::Property, publish::Publish, QoSLevel};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, default_value = "0", value_parser = QoSLevelParser)]
    qos: QoSLevel,
    #[arg(short, long, default_value = "hello-vaux")]
    topic: String,
    #[arg(short, long, default_value = "127.0.0.1")]
    addr: String,

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

    let addr: Ipv4Addr = args.addr.parse().expect("unable to create addr");
    let mut client = vaux_client::MqttClient::new(
        IpAddr::V4(addr),
        1883,
        "vaux-publisher-001",
        true,
        10,
        false,
    );
    match client.connect() {
        Ok(_) => {
            let handle = client.start();
            let producer = client.producer();
            //let mut receiver = client.take_consumer();

            let mut publish = Publish::default();
            publish
                .properties_mut()
                .set_property(Property::PayloadFormat(
                    vaux_mqtt::property::PayloadFormat::Utf8,
                ));
            publish.topic_name = Some(args.topic);
            publish.set_payload(Vec::from(args.message.as_bytes()));
            publish.set_qos(args.qos);
            publish.packet_id = Some(101);
            if producer.send(vaux_mqtt::Packet::Publish(publish)).is_err() {
                eprintln!("unable to send packet to broker");
            } else {
                println!("sent message");
            }
            client.stop();
            if handle.unwrap().join().unwrap().is_err() {
                eprintln!("error in client thread");
            }
        }
        Err(e) => eprintln!("{:#?}", e),
    }
}
