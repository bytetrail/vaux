use crate::{MqttConnection, MqttError};
use std::{collections::HashMap, fmt::Display, time::Duration};
use tokio::sync::mpsc::Sender;
use vaux_mqtt::{Packet, PacketType, WillMessage};

const DEFAULT_RECV_MAX: u16 = 100;
const DEFAULT_SESSION_EXPIRY: u32 = 1000;
const DEFAULT_MAX_PACKET_SIZE: usize = 64 * 1024;
const DEFAULT_CLIENT_KEEP_ALIVE: Duration = Duration::from_secs(60);
const MIN_KEEP_ALIVE: Duration = Duration::from_secs(30);
const MAX_CONNECT_WAIT: Duration = Duration::from_secs(5);
const DEFAULT_CLIENT_ID_PREFIX: &str = "vaux-client";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderError {
    ProducerRequired,
    ConsumerRequired,
    MinKeepAlive,
}

impl Display for BuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuilderError::ProducerRequired => write!(f, "producer channel is required"),
            BuilderError::ConsumerRequired => write!(f, "consumer channel is required"),
            BuilderError::MinKeepAlive => write!(f, "keep alive must be at least 30 seconds"),
        }
    }
}

pub struct ClientBuilder {
    connection: MqttConnection,
    auto_ack: bool,
    auto_packet_id: bool,
    receive_max: u16,
    session_expiry: u32,
    client_id: String,
    max_packet_size: usize,
    keep_alive: Duration,
    max_connect_wait: Duration,
    channel_size: Option<u16>,
    filtered_consumer: Option<HashMap<PacketType, Sender<vaux_mqtt::Packet>>>,
    error_out: Option<Sender<MqttError>>,
    will_message: Option<WillMessage>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            connection: MqttConnection::new(),
            auto_ack: true,
            auto_packet_id: true,
            receive_max: DEFAULT_RECV_MAX,
            session_expiry: DEFAULT_SESSION_EXPIRY,
            client_id: format!("{}-{}", DEFAULT_CLIENT_ID_PREFIX, uuid::Uuid::new_v4()),
            max_packet_size: DEFAULT_MAX_PACKET_SIZE,
            keep_alive: DEFAULT_CLIENT_KEEP_ALIVE,
            max_connect_wait: MAX_CONNECT_WAIT,
            channel_size: None,
            filtered_consumer: None,
            error_out: None,
            will_message: None,
        }
    }
}

impl ClientBuilder {
    pub fn new(connection: MqttConnection) -> Self {
        Self {
            connection,
            ..Default::default()
        }
    }

    /// Enables or disables automatic acknowledgement of packets. If enabled, the client
    /// will automatically acknowledge packets that require acknowledgement. If disabled,
    /// the client will not acknowledge packets that require acknowledgement and the
    /// calling client is responsible for acknowledging packets that require
    /// acknowledgement.
    pub fn with_auto_ack(mut self, auto_ack: bool) -> Self {
        self.auto_ack = auto_ack;
        self
    }

    /// Enables or disables automatic packet ID assignment. If enabled, the client will
    /// automatically assign packet IDs to packets that require them. If disabled, the
    /// client will not assign packet IDs to packets that require them. In some cases
    /// it may be desirable to disable automatic packet ID assignment, such as when
    /// using a custom packet ID assignment scheme or when the calling client is
    /// correlating packets with responses such as when `auto_ack` is disabled.
    ///
    /// Defaults to true.
    pub fn with_auto_packet_id(mut self, auto_packet_id: bool) -> Self {
        self.auto_packet_id = auto_packet_id;
        self
    }

    pub fn with_receive_max(mut self, receive_max: u16) -> Self {
        self.receive_max = receive_max;
        self
    }

    pub fn with_session_expiry(mut self, session_expiry: u32) -> Self {
        self.session_expiry = session_expiry;
        self
    }

    /// Sets the client ID for the MQTT client. The client ID is a unique identifier for
    /// the client and is used by the broker to identify the client. If the client ID is
    /// not set, a random client ID will be generated.
    /// Defaults to `vaux-client-{uuid}`.
    pub fn with_client_id(mut self, client_id: &str) -> Self {
        self.client_id = client_id.to_string();
        self
    }

    pub fn with_max_packet_size(mut self, max_packet_size: usize) -> Self {
        self.max_packet_size = max_packet_size;
        self
    }

    /// Sets the keep alive interval for the client. The keep alive interval is the
    /// maximum time that the client will wait between sending a PINGREQ packet to the
    /// broker. If the client does not send a PINGREQ packet within this time, the
    /// broker may assume that the client is no longer connected close the connection.
    pub fn with_keep_alive(mut self, keep_alive: Duration) -> Self {
        self.keep_alive = keep_alive;
        self
    }

    /// Sets the maximum time that the client will wait for a connection to be
    /// established. If the connection is not established within this time, the
    /// client will return an error.
    pub fn with_max_connect_wait(mut self, max_connect_wait: Duration) -> Self {
        self.max_connect_wait = max_connect_wait;
        self
    }

    /// Adds a filtered consumer to the client. The filtered consumer is used to
    /// receive packets of a specific type from the remote broker to the client of
    /// the MqttClient instance. The MQTT client will filter packets of the specified
    /// type and send them to the filtered consumer. Packets sent to a filtered consumer
    /// are not sent to the packet consumer.
    ///
    /// Filtered consumers do not have to be set for the client to be created.
    pub fn with_filtered_consumer(
        mut self,
        packet_type: PacketType,
        sender: Sender<Packet>,
    ) -> Self {
        if self.filtered_consumer.is_none() {
            self.filtered_consumer = Some(HashMap::new());
        }
        if let Some(consumer) = self.filtered_consumer.as_mut() {
            consumer.insert(packet_type, sender);
        }
        self
    }

    /// Sets the error handler for the MQTT client. The error handler is used to receive
    /// errors from the MQTT client. The error handler is not required to be set for the
    /// client to be created.
    pub fn with_error_handler(mut self, error_out: Sender<MqttError>) -> Self {
        self.error_out = Some(error_out);
        self
    }

    /// Sets the will message for the MQTT client. The will message is a message that
    /// the broker will send to the specified topic if the client disconnects
    /// unexpectedly. The will message is not required to be set for the client to be created.
    ///
    pub fn with_will_message(mut self, will_message: WillMessage) -> Self {
        self.will_message = Some(will_message);
        self
    }

    pub fn with_channel_size(mut self, channel_size: u16) -> Self {
        self.channel_size = Some(channel_size);
        self
    }

    pub fn build(self) -> Result<crate::MqttClient, BuilderError> {
        if self.keep_alive < MIN_KEEP_ALIVE {
            return Err(BuilderError::MinKeepAlive);
        }

        let mut client = crate::MqttClient::new_with_connection(
            self.connection,
            &self.client_id,
            self.auto_ack,
            self.receive_max,
            self.auto_packet_id,
            self.channel_size,
        );
        client.set_session_expiry(self.session_expiry);
        client.set_max_packet_size(self.max_packet_size);
        client.set_keep_alive(self.keep_alive);
        client.set_max_connect_wait(self.max_connect_wait);
        if let Some(error_out) = self.error_out {
            client.set_error_out(error_out);
        }
        client.set_will_message(self.will_message);
        Ok(client)
    }
}
