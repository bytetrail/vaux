use std::net::{SocketAddr, SocketAddrV4};

fn main() {
    let mut client = vaux_client::MQTTClient::new_with_id(
        SocketAddr::V4(SocketAddrV4::new(0x_7f_00_00_01.into(), 1833)),
        "vaux-20230101T1500CST",
    );
    match client.connect() {
        Ok(_) => {
            println!("connected");
        }
        Err(_) => todo!(),
    }
}
