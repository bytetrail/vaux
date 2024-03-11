use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

#[derive(Debug)]
pub struct MqttStream<'a> {
    tcp: Option<TcpStream>,
    tls: Option<rustls::Stream<'a, rustls::ClientConnection, TcpStream>>,
}

impl<'a> MqttStream<'a> {
    pub fn new_tcp(tcp: TcpStream) -> Self {
        Self {
            tcp: Some(tcp),
            tls: None,
        }
    }

    pub fn new_tls(tls_conn: &'a mut rustls::ClientConnection, tcp: &'a mut TcpStream) -> Self {
        Self {
            tcp: None,
            tls: Some(rustls::Stream::new(tls_conn, tcp)),
        }
    }

    pub fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        if let Some(ref mut tcp) = self.tcp {
            return tcp.set_read_timeout(timeout);
        }
        if let Some(ref mut tls) = self.tls {
            return tls.sock.set_read_timeout(timeout);
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "no stream available",
        ))
    }

    pub fn shutdown(&mut self) -> std::io::Result<()> {
        if let Some(ref mut tcp) = self.tcp {
            return tcp.shutdown(std::net::Shutdown::Both);
        }
        if let Some(ref mut tls) = self.tls {
            return tls.sock.shutdown(std::net::Shutdown::Both);
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "no stream available",
        ))
    }
}

impl<'a> Read for MqttStream<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Some(ref mut tcp) = self.tcp {
            return tcp.read(buf);
        }
        if let Some(ref mut tls) = self.tls {
            return tls.read(buf);
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "no stream available",
        ))
    }
}

impl<'a> Write for MqttStream<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(ref mut tcp) = self.tcp {
            return tcp.write(buf);
        }
        if let Some(ref mut tls) = self.tls {
            return tls.write(buf);
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "no stream available",
        ))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(ref mut tcp) = self.tcp {
            return tcp.flush();
        }
        if let Some(ref mut tls) = self.tls {
            return tls.flush();
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "no stream available",
        ))
    }
}
