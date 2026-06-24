pub mod packet;

pub use packet::{Error, PacketStream};

use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

#[derive(Debug)]
pub enum MqttStream {
    TcpStream(TcpStream),
    ClientTlsStream(Box<tokio_rustls::client::TlsStream<TcpStream>>),
    ServerTlsStream(Box<tokio_rustls::server::TlsStream<TcpStream>>),
}

#[derive(Debug)]
pub struct AsyncMqttStream(pub MqttStream);

impl AsyncRead for AsyncMqttStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut().0 {
            MqttStream::TcpStream(ref mut stream) => Pin::new(stream).poll_read(cx, buf),
            MqttStream::ClientTlsStream(ref mut stream) => Pin::new(stream).poll_read(cx, buf),
            MqttStream::ServerTlsStream(ref mut stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for AsyncMqttStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        match self.get_mut().0 {
            MqttStream::TcpStream(ref mut stream) => Pin::new(stream).poll_write(cx, buf),
            MqttStream::ClientTlsStream(ref mut stream) => Pin::new(stream).poll_write(cx, buf),
            MqttStream::ServerTlsStream(ref mut stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut().0 {
            MqttStream::TcpStream(ref mut stream) => Pin::new(stream).poll_flush(cx),
            MqttStream::ClientTlsStream(ref mut stream) => Pin::new(stream).poll_flush(cx),
            MqttStream::ServerTlsStream(ref mut stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut().0 {
            MqttStream::TcpStream(ref mut stream) => Pin::new(stream).poll_shutdown(cx),
            MqttStream::ClientTlsStream(ref mut stream) => Pin::new(stream).poll_shutdown(cx),
            MqttStream::ServerTlsStream(ref mut stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}
