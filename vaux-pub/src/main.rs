use std::net::{IpAddr, Ipv4Addr};

fn main() {
    let addr: Ipv4Addr = "127.0.0.1".parse().expect("unable to create addr");
    let mut client =
        vaux_client::MqttClient::new_with_id(IpAddr::V4(addr), 1883, "vaux-20230101T1500CST");
    match client.connect() {
        Ok(_) => {
            println!("connected");
            match client.send_utf8("hello-vaux", "vaux says, \"Hello, MQTT!\"", None) {
                Ok(_) => println!("sent"),
                Err(e) => eprintln!("{:#?}", e),
            }
        }
        Err(e) => eprintln!("{:#?}", e),
    }
}
