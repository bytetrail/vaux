use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

fn main() {
    let addr: Ipv4Addr = "127.0.0.1".parse().expect("unable to create addr");
    let mut client =
        vaux_client::MQTTClient::new_with_id(IpAddr::V4(addr), 1833, "vaux-20230101T1500CST");
    match client.connect() {
        Ok(_) => {
            println!("connected");
            match client.send("hello-vaux", "vaux says, \"Hello, MQTT!\"") {
                Ok(_) => println!("sent"),
                Err(e) => eprintln!("{:#?}", e),
            }
        }
        Err(e) => eprintln!("{:#?}", e),
    }
}
