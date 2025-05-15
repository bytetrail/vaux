use crate::{session::SessionState, MqttConnection, MqttError};
use std::{collections::HashMap, fmt::Display, sync::Arc, time::Duration};
use tokio::sync::{mpsc::Sender, Mutex, RwLock};
use vaux_mqtt::{Packet, PacketType, WillMessage};

const MIN_KEEP_ALIVE: Duration = Duration::from_secs(30);
const MAX_CONNECT_WAIT: Duration = Duration::from_secs(5);

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
    state: SessionState,
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
            state: SessionState::default(),
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

    pub fn with_state(mut self, connection: MqttConnection) -> Self {
        self.connection = connection;
        self
    }

    /// Enables or disables automatic acknowledgement of packets. If enabled, the client
    /// will automatically acknowledge packets that require acknowledgement. If disabled,
    /// the client will not acknowledge packets that require acknowledgement and the
    /// calling client is responsible for acknowledging packets that require
    /// acknowledgement.
    pub fn with_auto_ack(mut self, auto_ack: bool) -> Self {
        self.state.auto_ack = auto_ack;
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
        self.state.auto_packet_id = auto_packet_id;
        self
    }

    pub fn with_receive_max(mut self, receive_max: u16) -> Self {
        self.state.qos_recv_remaining = receive_max as usize;
        self
    }

    pub fn with_session_expiry(mut self, session_expiry: Duration) -> Self {
        self.state.session_expiry = Arc::new(RwLock::new(session_expiry));
        self
    }

    /// Sets the client ID for the MQTT client. The client ID is a unique identifier for
    /// the client and is used by the broker to identify the client. If the client ID is
    /// not set, a random client ID will be generated.
    /// Defaults to `vaux-client-{uuid}`.
    pub fn with_client_id(mut self, client_id: &str) -> Self {
        self.state.client_id = Arc::new(Mutex::new(Some(client_id.to_string())));
        self
    }

    pub fn with_max_packet_size(mut self, max_packet_size: usize) -> Self {
        self.state.max_packet_size = max_packet_size;
        self
    }

    /// Sets the keep alive interval for the client. The keep alive interval is the
    /// maximum time that the client will wait between sending a PINGREQ packet to the
    /// broker. If the client does not send a PINGREQ packet within this time, the
    /// broker may assume that the client is no longer connected close the connection.
    pub fn with_keep_alive(mut self, keep_alive: Duration) -> Self {
        self.state.keep_alive = Arc::new(RwLock::new(keep_alive));
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

    /// Sets the pingresp filter for the MQTT client. By default the client will not pass
    /// PINGRESP packets to the packet consumer. If with_pingresp is set to true, the
    /// client will pass PINGRESP packets to the packet consumer.
    pub fn with_pingresp(mut self, pingresp: bool) -> Self {
        self.state.pingresp = pingresp;
        self
    }

    pub async fn build(self) -> Result<crate::MqttClient, BuilderError> {
        let keep_alive = self.state.keep_alive.read().await;
        if *keep_alive < MIN_KEEP_ALIVE {
            return Err(BuilderError::MinKeepAlive);
        }
        drop(keep_alive);
        let mut client =
            crate::MqttClient::new_with_connection(self.connection, self.state, self.channel_size);
        client.set_max_connect_wait(self.max_connect_wait);
        if let Some(error_out) = self.error_out {
            client.set_error_out(error_out);
        }
        client.set_will_message(self.will_message);
        if let Some(filtered_consumer) = self.filtered_consumer {
            for (packet_type, sender) in filtered_consumer {
                client.add_filtered_packet_handler(packet_type, sender);
            }
        }
        Ok(client)
    }
}
