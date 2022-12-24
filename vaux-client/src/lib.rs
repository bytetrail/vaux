use std::{
    fmt::Error,
    net::{SocketAddr, TcpStream, ToSocketAddrs},
};

pub struct MQTTClient {
    addr: SocketAddr,
    client_id: Option<String>,
    connection: Option<TcpStream>,
}

impl MQTTClient {
    pub fn new(addr: SocketAddr) -> Self {
        MQTTClient {
            addr,
            client_id: None,
            connection: None,
        }
    }

    pub fn connect(&mut self) {
        match TcpStream::connect(self.addr) {
            Ok(s) => {
                self.connection = Some(s);
            }
            Err(_) => todo!(),
        }
    }

    pub fn disconnect(&mut self) {
        if let Some(_conn) = self.connection.as_mut() {

        } else {
            // it is an error to disconnect when not connected
        }

    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
