use std::net::Ipv4Addr;

use vaux_client::ErrorKind;

fn main() {
    let addr: Ipv4Addr = "127.0.0.1".parse().expect("unable to create addr");
    let mut client = vaux_client::MQTTClient::new_with_id(
        std::net::IpAddr::V4(addr),
        1833,
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
                    loop {
                        match client.read_next() {
                            Ok(next) => {
                                println!("{:#?}", next);
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
        Err(_) => todo!(),
    }
}
