use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

pub enum MqttStream {
    TcpStream(TcpStream),
    TlsStream(TlsStream<TcpStream>),
}

pub struct AsyncMqttStream(pub MqttStream);

impl AsyncRead for AsyncMqttStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut().0 {
            MqttStream::TcpStream(ref mut stream) => {
                let stream = Pin::new(stream);
                stream.poll_read(cx, buf)
            }
            MqttStream::TlsStream(ref mut stream) => {
                let stream = Pin::new(stream);
                stream.poll_read(cx, buf)
            }
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
            MqttStream::TcpStream(ref mut stream) => {
                let stream = Pin::new(stream);
                stream.poll_write(cx, buf)
            }
            MqttStream::TlsStream(ref mut stream) => {
                let stream = Pin::new(stream);
                stream.poll_write(cx, buf)
            }
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut().0 {
            MqttStream::TcpStream(ref mut stream) => {
                let stream = Pin::new(stream);
                stream.poll_flush(cx)
            }
            MqttStream::TlsStream(ref mut stream) => {
                let stream = Pin::new(stream);
                stream.poll_flush(cx)
            }
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut().0 {
            MqttStream::TcpStream(ref mut stream) => {
                let stream = Pin::new(stream);
                stream.poll_shutdown(cx)
            }
            MqttStream::TlsStream(ref mut stream) => {
                let stream = Pin::new(stream);
                stream.poll_shutdown(cx)
            }
        }
    }
}
