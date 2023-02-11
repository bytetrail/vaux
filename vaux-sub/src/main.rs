use std::net::Ipv4Addr;

use vaux_client::ErrorKind;
use vaux_mqtt::Packet;
use std::io::Write;

fn main() {
    let addr: Ipv4Addr = "127.0.0.1".parse().expect("unable to create addr");
    let mut client = vaux_client::MQTTClient::new_with_id(
        std::net::IpAddr::V4(addr),
        1883,
        "vaux-subscriber-001",
    );
    match client.connect() {
        Ok(_) => {
            println!("connected");
            match client.subscribe(1, "hello-vaux") {
                Ok(_) => {
                    if client.set_read_timeout(1000).is_err() {
                        eprintln!("Unable to set read timeout");
                        return;
                    }
                    let mut packet_count = 0;
                    loop {
                        match client.read_next() {
                            Ok(next) => {
                                match next {
                                    Some(packet) => {
                                        packet_count += 1;
                                        match packet {
                                            Packet::Publish(mut p) => {
                                                print!("{:?}", p.take_payload().unwrap());
                                                std::io::stdout().flush().expect("unable to flush stdout");
                                            }
                                            _ => {}
                                        }
                                        if packet_count == 80 {
                                            println!();
                                            packet_count = 0;
                                        }
                                    }
                                    None => {}
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
