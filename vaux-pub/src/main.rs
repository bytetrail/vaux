use std::net::{IpAddr, Ipv4Addr};

use clap::{Parser, error::ErrorKind};
use vaux_mqtt::QoSLevel;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, default_value = "0", value_parser = QoSLevelParser)]
    qos: QoSLevel,
    #[arg(short, long, default_value = "127.0.0.1")]
    addr: String,
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
            _ => Err(clap::Error::new(ErrorKind::InvalidValue))
        }
    }

}

fn main() {
    let args = Args::parse();

    let addr: Ipv4Addr = args.addr.parse().expect("unable to create addr");
    let mut client =
        vaux_client::MqttClient::new_with_id(IpAddr::V4(addr), 1883, "vaux-20230101T1500CST");
    match client.connect() {
        Ok(_) => {
            println!("connected");
            if let Err(e) = client.publish(
                "hello-vaux",
                "Vaux says, \"Hello MQTT!\"".as_bytes(),
                args.qos,
                None,
            ) {
                eprintln!("{:#?}", e)
            } else {
                println!("sent")
            }
        }
        Err(e) => eprintln!("{:#?}", e),
    }
}
