use std::net::{SocketAddr, SocketAddrV4};

fn main() {
    let mut client = vaux_client::MQTTClient::new_with_id(
        SocketAddr::V4(SocketAddrV4::new(0x_7f_00_00_01.into(), 1833)),
        "vaux-subscriber-001",
    );
    match client.connect() {
        Ok(_) => {
            println!("connected");
            match client.subscribe(1, "hello-vaux") {
                Ok(_) => {
                    loop {
                        match client.read_next() {
                            Ok(next) => {
                                println!("{:#?}", next);
                            }
                            Err(e) => eprintln!("{:#?}", e),
                        }
                    }
                }
                Err(e) => eprintln!("{:#?}", e),
            }
        }
        Err(_) => todo!(),
    }
}
