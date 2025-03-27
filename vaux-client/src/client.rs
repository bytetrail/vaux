use crate::session::ClientSession;
use crate::{ErrorKind, MqttConnection, MqttError};
use async_std::sync::RwLock;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::select;
use tokio::{
    sync::{
        mpsc::{self, error::SendError, Receiver, Sender},
        Mutex,
    },
    task::JoinHandle,
};
use vaux_mqtt::{Packet, PacketType, QoSLevel, Subscribe, SubscriptionFilter};

const DEFAULT_RECV_MAX: u16 = 100;
const DEFAULT_SESSION_EXPIRY: Duration = Duration::from_secs(1000);
// 64K is the default max packet size
const DEFAULT_MAX_PACKET_SIZE: usize = 64 * 1024;
const DEFAULT_CLIENT_KEEP_ALIVE: Duration = Duration::from_secs(60);
const MIN_KEEP_ALIVE: Duration = Duration::from_secs(30);
const MAX_CONNECT_WAIT: Duration = Duration::from_secs(5);
const DEFAULT_CHANNEL_SIZE: u16 = 128;

pub type FilteredChannel = HashMap<PacketType, Sender<vaux_mqtt::Packet>>;

pub struct PacketChannel(
    Sender<vaux_mqtt::Packet>,
    Option<Receiver<vaux_mqtt::Packet>>,
);

impl PacketChannel {
    pub fn new() -> Self {
        let (sender, receiver): (Sender<vaux_mqtt::Packet>, Receiver<vaux_mqtt::Packet>) =
            mpsc::channel(DEFAULT_CHANNEL_SIZE as usize);
        Self(sender, Some(receiver))
    }

    pub fn new_with_size(size: usize) -> Self {
        let (sender, receiver): (Sender<vaux_mqtt::Packet>, Receiver<vaux_mqtt::Packet>) =
            mpsc::channel(size);
        Self(sender, Some(receiver))
    }

    pub fn new_from_channel(
        sender: Sender<vaux_mqtt::Packet>,
        receiver: Receiver<vaux_mqtt::Packet>,
    ) -> Self {
        Self(sender.clone(), Some(receiver))
    }

    pub fn sender(&self) -> Sender<vaux_mqtt::Packet> {
        self.0.clone()
    }

    pub fn take_receiver(&mut self) -> Option<Receiver<vaux_mqtt::Packet>> {
        self.1.take()
    }
}

impl Default for PacketChannel {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MqttClient {
    pub(crate) connection: Option<MqttConnection>,
    pub(crate) auto_ack: bool,
    pub(crate) receive_max: u16,
    pub(crate) auto_packet_id: bool,
    filter_channel: FilteredChannel,
    connected: Arc<RwLock<bool>>,
    pub(crate) session_expiry: Arc<RwLock<Duration>>,
    pub(crate) client_id: Arc<Mutex<Option<String>>>,
    pub(crate) packet_in: PacketChannel,
    pub(crate) packet_out: PacketChannel,
    pub(crate) err_chan: Option<Sender<MqttError>>,
    pub(crate) max_packet_size: usize,
    pub(crate) keep_alive: Arc<RwLock<Duration>>,
    max_connect_wait: Duration,
    pub(crate) will_message: Option<vaux_mqtt::WillMessage>,
    pub(crate) pingresp: bool,
}

impl Default for MqttClient {
    fn default() -> Self {
        Self::new(
            &uuid::Uuid::new_v4().to_string(),
            true,
            DEFAULT_RECV_MAX,
            true,
            None,
        )
    }
}

impl MqttClient {
    pub(crate) fn new_with_connection(
        connection: MqttConnection,
        client_id: &str,
        auto_ack: bool,
        receive_max: u16,
        auto_packet_id: bool,
        channel_size: Option<u16>,
    ) -> Self {
        let mut client = Self::new(
            client_id,
            auto_ack,
            receive_max,
            auto_packet_id,
            channel_size,
        );
        client.connection = Some(connection);
        client
    }

    pub(crate) fn new(
        client_id: &str,
        auto_ack: bool,
        receive_max: u16,
        auto_packet_id: bool,
        channel_size: Option<u16>,
    ) -> Self {
        Self {
            connection: None,
            auto_ack,
            auto_packet_id,
            receive_max,
            connected: Arc::new(RwLock::new(false)),
            session_expiry: Arc::new(RwLock::new(DEFAULT_SESSION_EXPIRY)),
            client_id: Arc::new(Mutex::new(Some(client_id.to_string()))),
            filter_channel: HashMap::new(),
            packet_in: PacketChannel::new_with_size(
                channel_size.unwrap_or(DEFAULT_CHANNEL_SIZE) as usize
            ),
            packet_out: PacketChannel::new_with_size(
                channel_size.unwrap_or(DEFAULT_CHANNEL_SIZE) as usize
            ),
            err_chan: None,
            max_packet_size: DEFAULT_MAX_PACKET_SIZE,
            keep_alive: Arc::new(RwLock::new(DEFAULT_CLIENT_KEEP_ALIVE)),
            max_connect_wait: MAX_CONNECT_WAIT,
            will_message: None,
            pingresp: false,
        }
    }

    pub fn pingresp(&self) -> bool {
        self.pingresp
    }

    pub(crate) fn set_pingresp(&mut self, pingresp: bool) {
        self.pingresp = pingresp;
    }

    pub fn packet_producer(&mut self) -> Sender<Packet> {
        self.packet_in.sender()
    }

    pub fn take_packet_consumer(&mut self) -> Option<Receiver<Packet>> {
        self.packet_out.1.take()
    }

    pub(crate) fn set_max_packet_size(&mut self, max_packet_size: usize) {
        self.max_packet_size = max_packet_size;
    }

    pub async fn connected(&self) -> bool {
        *self.connected.read().await
    }

    pub async fn session_expiry(&self) -> Duration {
        *self.session_expiry.read().await
    }

    pub(crate) async fn set_session_expiry(&mut self, session_expiry: u32) {
        let mut _session_expiry = self.session_expiry.write().await;
        *_session_expiry = Duration::from_secs(session_expiry as u64);
    }

    pub(crate) fn set_error_out(&mut self, error_out: Sender<MqttError>) {
        self.err_chan = Some(error_out);
    }

    pub(crate) async fn set_keep_alive(&mut self, keep_alive: Duration) {
        let mut _keep_alive = self.keep_alive.write().await;
        if keep_alive < MIN_KEEP_ALIVE {
            *_keep_alive = MIN_KEEP_ALIVE;
        } else {
            *_keep_alive = keep_alive;
        }
    }

    pub async fn keep_alive(&self) -> Duration {
        *self.keep_alive.read().await
    }

    /// Gets the maximum connection wait time. This is the maximum time that
    /// the client will wait for a connection to be established with the
    /// remote broker before returning an error. The default maximum
    /// connection wait time is 5 seconds.
    pub fn max_connect_wait(&self) -> Duration {
        self.max_connect_wait
    }

    pub(crate) fn set_max_connect_wait(&mut self, max_connect_wait: Duration) {
        self.max_connect_wait = max_connect_wait;
    }

    pub(crate) fn set_will_message(&mut self, will_message: Option<vaux_mqtt::WillMessage>) {
        self.will_message = will_message;
    }

    /// Helper method to ping the broker.
    ///
    pub async fn ping(&mut self) -> std::result::Result<(), SendError<Packet>> {
        let ping = vaux_mqtt::Packet::PingRequest(Default::default());
        self.packet_in.0.send(ping).await
    }

    pub async fn publish(
        &mut self,
        topic: Option<String>,
        topic_alias: Option<u16>,
        qos: QoSLevel,
        payload: Vec<u8>,
        utf8: bool,
    ) -> std::result::Result<(), SendError<Packet>> {
        let mut publish = vaux_mqtt::publish::Publish::default();
        publish.topic_name = topic;
        publish.set_payload(payload);
        publish
            .properties_mut()
            .set_property(vaux_mqtt::property::Property::PayloadFormat(if utf8 {
                vaux_mqtt::property::PayloadFormat::Utf8
            } else {
                vaux_mqtt::property::PayloadFormat::Bin
            }));
        match qos {
            QoSLevel::AtLeastOnce => {
                publish.header.set_dup(true);
            }
            QoSLevel::ExactlyOnce => {
                publish.header.set_dup(false);
            }
            _ => {
                publish.header.set_dup(false);
            }
        }
        publish.set_qos(qos);

        if let Some(alias) = topic_alias {
            publish.set_topic_alias(alias);
        }
        self.packet_out
            .0
            .send(vaux_mqtt::Packet::Publish(publish))
            .await
    }

    /// Helper method to publish a message to the given topic. This helper
    /// method will publish the message with a QoS level of "At Most Once",
    /// or 0.
    pub async fn publish_str(
        &mut self,
        topic: &str,
        payload: &str,
    ) -> std::result::Result<(), SendError<Packet>> {
        self.publish(
            Some(topic.to_string()),
            None,
            QoSLevel::AtMostOnce,
            payload.as_bytes().to_vec(),
            true,
        )
        .await
    }

    /// Helper method to subscribe to the topics in the topic filter. This helper
    /// subscribes with a QoS level of "At Most Once", or 0. A SUBACK will
    /// typically be returned on the consumer on a successful subscribe.
    pub async fn subscribe(
        &mut self,
        packet_id: u16,
        topic_filter: &[&str],
        qos: QoSLevel,
    ) -> std::result::Result<(), SendError<Packet>> {
        let mut subscribe = Subscribe::default();
        subscribe.set_packet_id(packet_id);
        for topic in topic_filter {
            let subscription = SubscriptionFilter {
                filter: (*topic).to_string(),
                qos,
                ..Default::default()
            };
            // self.subscriptions.push(subscription.clone());
            subscribe.add_subscription(subscription);
        }
        self.packet_in
            .0
            .send(vaux_mqtt::Packet::Subscribe(subscribe))
            .await
    }

    /// Attempts to start an MQTT session with the remote broker. The client will
    /// attempt to connect to the remote broker and send a CONNECT packet. If the
    /// client is unable to connect to the remote broker, an error will be returned.
    /// The ```max_wait``` parameter is used to determine how long the client will
    /// wait for the connection to be established. If the connection is not established
    /// within the ```max_wait``` interval, an error will be returned.
    /// Example:
    /// ```
    /// use vaux_client::MqttClient;
    /// use vaux_client::MqttConnection;
    /// use std::time::Duration;
    /// use vaux_client::ClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let conn = MqttConnection::new().with_host("localhost").with_port(1883);
    ///     let mut producer = vaux_client::PacketChannel::new();
    ///     let consumer = vaux_client::PacketChannel::new();
    ///     let mut client = ClientBuilder::new(conn).
    ///         with_client_id("test-client")
    ///        .build().await.unwrap();
    ///
    ///     let handle: Option<tokio::task::JoinHandle<_>> =
    ///     match client.try_start(Duration::from_millis(5000), true).await {
    ///         Ok(h) => Some(h),
    ///         Err(e) => None,
    ///     };
    /// }
    /// ```
    ///
    pub async fn try_start(
        &mut self,
        max_wait: Duration,
        clean_start: bool,
    ) -> crate::Result<JoinHandle<crate::Result<()>>> {
        let handle = self.start(clean_start).await;
        match tokio::time::timeout(max_wait, {
            async {
                while !self.connected().await {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        })
        .await
        {
            Ok(_) => handle,
            Err(e) => {
                self.stop().await?;
                Err(MqttError::new(
                    &format!("unable to connect to broker: {}", e),
                    ErrorKind::Transport,
                ))
            }
        }
    }

    /// Starts the MQTT client thread. The MQTT client thread will send packets
    /// to the remote broker that it receives on the producer channel and make
    /// packets available on the consumer channel that it receives from the broker
    ///
    /// The MQTT client thread can be stopped by calling the stop method or by
    /// sending a DISCONNECT packet on the producer channel.
    ///
    /// There are cases where the client may not be able to send a message (e.g.
    /// QoS 1 and no more messages can be sent). In these cases, the message will
    /// be queued and sent when the client is able to send it or until the maximum
    /// queue size is reached based on packet size and/or count. The client will
    /// thread will terminate if the queue is full and the client is unable to send.
    ///
    /// Queued messages will be sent in the order they were received. Any messages
    /// that are queued when the client is stopped will remain queued until the client
    /// is started again or the client is dropped.
    pub async fn start(
        &mut self,
        clean_start: bool,
    ) -> Result<JoinHandle<crate::Result<()>>, MqttError> {
        if self.connection.is_none() {
            return Err(MqttError::new("connection not set", ErrorKind::Connection));
        }
        let max_connect_wait = self.max_connect_wait;
        let will_message = self.will_message.clone();
        let mut packet_in_receiver = self.packet_in.1.take().ok_or(MqttError::new(
            "packet_in channel closed",
            ErrorKind::Transport,
        ))?;
        let packet_out = self.packet_out.0.clone();
        let credentials = self.connection.as_ref().unwrap().credentials().clone();
        let connected = self.connected.clone();
        let filter_channel = self.filter_channel.clone();
        if let Some(connection) = self.connection.as_mut() {
            match connection.connect().await {
                Ok(c) => c,
                Err(e) => {
                    return Err(MqttError::new(
                        &format!("unable to connect to broker socket: {}", e),
                        ErrorKind::Transport,
                    ));
                }
            };
        }
        let mut session = match ClientSession::new_from_client(self).await {
            Ok(s) => s,
            Err(e) => {
                return Err(MqttError::new(
                    &format!("unable to create client session: {}", e),
                    ErrorKind::Transport,
                ));
            }
        };
        let _keep_alive = self.keep_alive.clone();
        let _session_expiry = self.session_expiry.clone();
        Ok(tokio::spawn(async move {
            match session
                .connect(max_connect_wait, credentials, clean_start, will_message)
                .await
            {
                Ok(_) => *connected.write().await = true,
                Err(e) => {
                    *connected.write().await = false;
                    return Err(e);
                }
            }
            // get the session keep alive duration set after the connect
            let keep_alive = session.keep_alive();
            // set the client keep alive duration
            {
                *_keep_alive.write().await = keep_alive;
            }
            // get the session expiry duration set after the connect
            let session_expiry = session.session_expiry();
            {
                *_session_expiry.write().await = session_expiry;
            }
            loop {
                select! {
                    _ = MqttClient::keep_alive_timer(keep_alive) => {
                        match session.handle_keep_alive().await {
                            Ok(_) => {
                                // do nothing
                            }
                            Err(e) => {
                                *connected.write().await = false;
                                return Err(e);
                            }
                        }
                    }
                    packet_result = session.read_next() => {
                        match packet_result {
                            Ok(Some(packet)) => {
                                match session.handle_packet(packet).await {
                                    Ok(Some(packet)) => {
                                        // check filter channels
                                        if let Some(filter_out) = filter_channel.get(&PacketType::from(&packet)) {
                                            if let Err(e) = filter_out.send(packet.clone()).await {
                                                *connected.write().await = false;
                                                return Err(MqttError::new(
                                                    &format!("unable to send packet: {}", e),
                                                    ErrorKind::Transport,
                                                ));
                                            }
                                        } else {
                                            packet_out.send(packet).await.map_err(|e| {
                                                MqttError::new(
                                                    &format!("unable to send packet: {}", e),
                                                    ErrorKind::Transport,
                                                )
                                            })?;
                                        }
                                    }
                                    Ok(None) => {
                                        // do nothing
                                    }
                                    Err(e) => {
                                        *connected.write().await = false;
                                        return Err(e);
                                    }
                                }
                            }
                            Ok(None) => {
                                // socket closed
                                *connected.write().await = false;
                                return Err(MqttError::new("socket closed", ErrorKind::Transport));
                            }
                            Err(e) => {
                                *connected.write().await = false;
                                return Err(e);
                            }
                        }
                    }
                    pending_packet = packet_in_receiver.recv() => {
                        match pending_packet {
                            Some(inbound_packet) => {
                                // if auto-ack and a QOS control packet, ignore
                                if MqttClient::qos_control(&inbound_packet) && session.auto_ack() {
                                    continue;
                                }
                                match session.write_next(inbound_packet).await {
                                    Ok(_) => {
                                    }
                                    Err(e) => {
                                        *connected.write().await = false;
                                        return Err(e);
                                    }
                                }
                            }
                            None => {
                                *connected.write().await = false;
                                return Err(MqttError::new("packet_in channel closed", ErrorKind::Transport));
                            }
                        }
                    }
                };
                if !session.connected().await {
                    *connected.write().await = false;
                    return Ok(());
                }
            }
        }))
    }

    pub async fn stop(&mut self) -> Result<(), MqttError> {
        let disconnect = Packet::Disconnect(Default::default());
        if let Err(e) = self.packet_in.0.send(disconnect).await {
            return Err(MqttError::new(
                &format!("unable to send disconnect: {}", e),
                ErrorKind::Transport,
            ));
        }
        Ok(())
    }

    async fn keep_alive_timer(keep_alive: Duration) {
        tokio::time::sleep(keep_alive).await;
    }

    fn qos_control(packet: &Packet) -> bool {
        matches!(
            packet,
            Packet::PubAck(_) | Packet::PubRec(_) | Packet::PubRel(_) | Packet::PubComp(_)
        )
    }
}
