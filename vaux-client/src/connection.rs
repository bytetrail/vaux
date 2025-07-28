use async_std::net::ToSocketAddrs;
use rustls::pki_types::ServerName;
use std::{sync::Arc, time::Duration};
use tokio::net::TcpStream;
use tokio_rustls::{rustls::RootCertStore, TlsConnector};
use vaux_async::stream::{AsyncMqttStream, MqttStream};

#[cfg(feature = "developer")]
use crate::developer;
use crate::{ErrorKind, MqttError};

const DEFAULT_HOST: &str = "localhost";
pub const DEFAULT_PORT: u16 = 1883;
pub const DEFAULT_SECURE_PORT: u16 = 8883;
const DEFAULT_CONNECTION_TIMEOUT: u64 = 30_000;

pub struct MqttConnection {
    host: String,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
    pub(crate) tls: bool,
    trusted_ca: Option<Arc<RootCertStore>>,
    #[cfg(feature = "developer")]
    verifier: developer::Verifier,
    stream: Option<AsyncMqttStream>,
}

impl Default for MqttConnection {
    fn default() -> Self {
        Self::new()
    }
}

impl MqttConnection {
    pub fn new() -> Self {
        Self {
            username: None,
            password: None,
            host: DEFAULT_HOST.to_string(),
            port: None,
            tls: false,
            trusted_ca: None,
            #[cfg(feature = "developer")]
            verifier: developer::Verifier,
            stream: None,
        }
    }

    pub(crate) fn credentials(&self) -> Option<(String, String)> {
        if let Some(username) = &self.username {
            if let Some(password) = &self.password {
                return Some((username.clone(), password.clone()));
            }
        }
        None
    }

    pub fn with_credentials(mut self, username: &str, password: &str) -> Self {
        self.username = Some(username.to_string());
        self.password = Some(password.to_string());
        self
    }

    pub fn with_tls(mut self) -> Self {
        self.tls = true;
        if self.port.is_none() {
            self.port = Some(DEFAULT_SECURE_PORT);
        }
        self
    }

    pub fn with_host(mut self, host: &str) -> Self {
        self.host = host.to_string();
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn with_trust_store(mut self, trusted_ca: Arc<rustls::RootCertStore>) -> Self {
        self.trusted_ca = Some(trusted_ca);
        self
    }

    pub fn take_stream(&mut self) -> Option<AsyncMqttStream> {
        self.stream.take()
    }

    pub(crate) async fn connect(&mut self) -> crate::Result<()> {
        self.connect_with_timeout(Duration::from_millis(DEFAULT_CONNECTION_TIMEOUT))
            .await?;
        Ok(())
    }

    pub(crate) async fn connect_with_timeout(&mut self, timeout: Duration) -> crate::Result<()> {
        // if not set via with_tls or with_port, set the port to the default
        if self.port.is_none() {
            self.port = Some(DEFAULT_PORT);
        }
        let addr = self.host.clone() + ":" + &self.port.unwrap().to_string();
        let socket_addr = addr.to_socket_addrs().await.map_err(|e| {
            MqttError::new(
                &format!("unable to resolve host: {e}"),
                ErrorKind::Connection,
            )
        });
        if let Err(e) = socket_addr {
            return Err(MqttError::new(
                &format!("unable to resolve host: {e}"),
                ErrorKind::Connection,
            ));
        }
        let socket_addr = socket_addr.unwrap().next().unwrap();

        match tokio::time::timeout(timeout, TcpStream::connect(&socket_addr)).await {
            Ok(result) => match result {
                Ok(stream) => {
                    if self.tls {
                        self.stream = Some(self.connect_tls(stream).await?);
                        Ok(())
                    } else {
                        self.stream = Some(AsyncMqttStream(MqttStream::TcpStream(stream)));
                        Ok(())
                    }
                }
                Err(e) => Err(MqttError::new(
                    &format!("unable to connect: {e}"),
                    ErrorKind::Connection,
                )),
            },
            Err(_e) => Err(MqttError {
                message: "timeout".to_string(),
                kind: ErrorKind::Timeout,
            }),
        }
    }

    async fn connect_tls(&mut self, stream: TcpStream) -> crate::Result<AsyncMqttStream> {
        let server_name = ServerName::try_from(self.host.clone()).map_err(|e| {
            MqttError::new(
                &format!("unable to convert host to server name: {e}"),
                ErrorKind::Connection,
            )
        })?;
        if let Some(ca) = self.trusted_ca.clone() {
            let mut config = rustls::ClientConfig::builder()
                .with_root_certificates(ca)
                .with_no_client_auth();
            config.key_log = Arc::new(rustls::KeyLogFile::new());
            #[cfg(feature = "developer")]
            {
                self.verifier = developer::Verifier;
                config
                    .dangerous()
                    .set_certificate_verifier(Arc::new(self.verifier.clone()));
            }
            let connector = TlsConnector::from(Arc::new(config));
            let stream = connector.connect(server_name, stream).await.map_err(|e| {
                MqttError::new(
                    &format!("unable to establish TLS connection: {e}"),
                    ErrorKind::Connection,
                )
            })?;
            Ok(AsyncMqttStream(MqttStream::TlsStream(Box::new(stream))))
        } else {
            Err(MqttError::new(
                "no trusted CA(s) provided for TLS connection",
                ErrorKind::Connection,
            ))
        }
    }
}
