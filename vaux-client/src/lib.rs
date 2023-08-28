mod client;

use std::fmt::Display;

pub use client::MqttClient;
use vaux_mqtt::Reason;

#[derive(Default, Debug, PartialEq, Eq, Copy, Clone)]
#[repr(u8)]
pub enum ErrorKind {
    #[default]
    Codec,
    Protocol,
    IO,
    Connection(Reason),
    Timeout,
    Transport,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            ErrorKind::Codec => "Codec",
            ErrorKind::Protocol => "Protocol",
            ErrorKind::IO => "IO",
            ErrorKind::Connection(_r) => "Connection",
            ErrorKind::Timeout => "Timeout",
            ErrorKind::Transport => "Transport",
        };

        write!(f, "{}", kind)
    }
}

#[derive(Default, Debug)]
pub struct MqttError {
    message: String,
    kind: ErrorKind,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
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
            source: None,
        }
    }

    pub fn new_with_source(
        message: &str,
        kind: ErrorKind,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        Self {
            message: message.to_string(),
            kind,
            source: Some(source),
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn source(&self) -> Option<&(dyn std::error::Error + Send + Sync + 'static)> {
        self.source.as_ref().map(|s| s.as_ref() as _)
    }
}

pub type Result<T> = core::result::Result<T, MqttError>;
