use std::net::Ipv4Addr;

use std::io::Write;
use vaux_client::ErrorKind;
use vaux_mqtt::{Packet, PubResp, Subscription, Subscribe};

fn main() {
    let addr: Ipv4Addr = "127.0.0.1".parse().expect("unable to create addr");
    let mut client = vaux_client::MqttClient::new_with_id(
        std::net::IpAddr::V4(addr),
        1883,
        "vaux-subscriber-001",
    );
    match client.connect() {
        Ok(_) => {
            println!("connected");
            let filter = vec![
                // inbound device ops messages for this shadow on this site
                Subscription {
                    filter: "hello-vaux".to_string(),
                    qos: vaux_mqtt::QoSLevel::AtLeastOnce,
                    no_local: false,
                    retain_as: false,
                    handling: vaux_mqtt::subscribe::RetainHandling::None,
                },
            ];    
            let subscribe = Subscribe::new(1, Some(1000), filter);
            match client.send(Packet::Subscribe(subscribe)) {
                Ok(_) => {
                    if client.set_read_timeout(1000).is_err() {
                        eprintln!("Unable to set read timeout");
                        return;
                    }
                    let mut packet_count = 0;
                    loop {
                        match client.read_next() {
                            Ok(next) => {
                                if let Some(packet) = next {
                                    packet_count += 1;
                                    if let Packet::Publish(mut p) = packet {
                                        print!("{:?}", p.take_payload().unwrap());
                                        std::io::stdout().flush().expect("unable to flush stdout");
                                        let mut ack = PubResp::new_puback();
                                        ack.packet_id = p.packet_id.unwrap();
                                        if let Err(e) = client.send(Packet::PubAck(ack)) {
                                            eprintln!("{:?}", e);
                                        }
                                    }
                                    if packet_count == 80 {
                                        println!();
                                        packet_count = 0;
                                    }
                                }
                            }
                            Err(e) => match e.kind() {
                                ErrorKind::Timeout => {}
                                _ => {
                                    eprintln!("{}", e.message());
                                    break;
                                }
                            },
                        }
                    }
                }
                Err(e) => eprintln!("{:#?}", e),
            }
        }
        Err(e) => eprintln!("Unable to connect {}", e.message()),
    }
}
