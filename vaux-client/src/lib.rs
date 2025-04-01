mod builder;
mod client;
mod connection;
#[cfg(feature = "developer")]
mod developer;
mod session;

pub use builder::ClientBuilder;
pub use client::{MqttClient, PacketChannel};
pub use connection::MqttConnection;
use std::fmt::Display;
use vaux_mqtt::Reason;

pub type Result<T> = core::result::Result<T, MqttError>;

#[derive(Default, Debug, PartialEq, Eq, Copy, Clone)]
#[repr(u8)]
pub enum ErrorKind {
    #[default]
    Codec,
    Protocol(Reason),
    IO,
    Connection,
    Timeout,
    Transport,
    Session,
    Channel,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            ErrorKind::Codec => "Codec",
            ErrorKind::Protocol(_r) => "Protocol",
            ErrorKind::IO => "IO",
            ErrorKind::Connection => "Connection",
            ErrorKind::Timeout => "Timeout",
            ErrorKind::Transport => "Transport",
            ErrorKind::Session => "Session",
            ErrorKind::Channel => "Channel",
        };

        write!(f, "{}", kind)
    }
}

#[derive(Default, Debug, Clone)]
pub struct MqttError {
    message: String,
    kind: ErrorKind,
}

impl std::error::Error for MqttError {}

impl std::fmt::Display for MqttError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl MqttError {
    pub fn new(message: &str, kind: ErrorKind) -> Self {
        Self {
            message: message.to_string(),
            kind,
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}
