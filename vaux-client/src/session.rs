use bytes::BytesMut;
use std::{
    io::Read,
    net::{SocketAddr, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};
use vaux_mqtt::{decode, Packet};

use crate::MQTTResult;

pub struct Config {
    addr: SocketAddr,
    read_timeout: Duration,
}

struct SessionState {
    conn: Option<TcpStream>,
}

struct Session {}

impl Session {
    pub fn connect(config: Config) -> (Sender<Packet>, Receiver<MQTTResult<Packet>>) {
        let (prod_tx, prod_rx) = mpsc::channel();
        let (cons_tx, cons_rx) = mpsc::channel();

        let session_state = SessionState { conn: None };

        thread::spawn(move || {
            let mut buffer = [0u8; 128];
            let mut dest = BytesMut::default();
            loop {
                match session_state.conn.as_ref().unwrap().read(&mut buffer) {
                    Ok(len) => match decode(&mut BytesMut::from(&buffer[0..len])) {
                        Ok(p) => {}
                        Err(_) => todo!(),
                    },
                    Err(_) => todo!(),
                }
            }
        });

        (prod_tx, cons_rx)
    }
}
