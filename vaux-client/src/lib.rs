mod client;

pub use client::MqttClient;

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorKind {
    #[default]
    Codec,
    Protocol,
    Connection,
    Timeout,
    Transport,
}

#[derive(Default, Debug)]
pub struct MqttError {
    message: String,
    kind: ErrorKind,
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

pub type Result<T> = core::result::Result<T, MqttError>;
