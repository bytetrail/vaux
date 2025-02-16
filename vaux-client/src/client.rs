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
const DEFAULT_SESSION_EXPIRY: u32 = 1000;
// 64K is the default max packet size
const DEFAULT_MAX_PACKET_SIZE: usize = 64 * 1024;
const DEFAULT_CLIENT_KEEP_ALIVE: Duration = Duration::from_secs(60);
const MIN_KEEP_ALIVE: Duration = Duration::from_secs(30);
const MAX_CONNECT_WAIT: Duration = Duration::from_secs(5);
const DEFAULT_CHANNEL_SIZE: usize = 128;

pub type FilteredChannel = HashMap<PacketType, Sender<vaux_mqtt::Packet>>;

pub struct PacketChannel(
    Sender<vaux_mqtt::Packet>,
    Option<Receiver<vaux_mqtt::Packet>>,
);

impl PacketChannel {
    pub fn new() -> Self {
        let (sender, receiver): (Sender<vaux_mqtt::Packet>, Receiver<vaux_mqtt::Packet>) =
            mpsc::channel(DEFAULT_CHANNEL_SIZE);
        Self(sender, Some(receiver))
    }

    pub fn new_from_channel(
        sender: Sender<vaux_mqtt::Packet>,
        receiver: Receiver<vaux_mqtt::Packet>,
    ) -> Self {
        Self(sender.clone(), Some(receiver))
    }

    pub fn new_with_size(size: usize) -> Self {
        let (sender, receiver): (Sender<vaux_mqtt::Packet>, Receiver<vaux_mqtt::Packet>) =
            mpsc::channel(size);
        Self(sender, Some(receiver))
    }

    pub fn sender(&self) -> Sender<vaux_mqtt::Packet> {
        self.0.clone()
    }

    pub fn take_receiver(&mut self) -> Receiver<vaux_mqtt::Packet> {
        self.1.take().unwrap()
    }
}

pub struct MqttClient {
    pub(crate) connection: Option<MqttConnection>,
    pub(crate) auto_ack: bool,
    pub(crate) receive_max: u16,
    pub(crate) auto_packet_id: bool,
    filter_channel: FilteredChannel,
    connected: Arc<RwLock<bool>>,
    //last_error: Arc<Mutex<Option<MqttError>>>,
    session_expiry: u32,
    pub(crate) client_id: Arc<Mutex<Option<String>>>,
    pub(crate) packet_in: Option<PacketChannel>,
    pub(crate) packet_out: Option<Sender<vaux_mqtt::Packet>>,
    pub(crate) err_chan: Option<Sender<MqttError>>,
    pending_qos1: Arc<Mutex<Vec<Packet>>>,
    pub(crate) max_packet_size: usize,
    pub(crate) keep_alive: Duration,
    max_connect_wait: Duration,
}

impl Default for MqttClient {
    fn default() -> Self {
        Self::new(
            &uuid::Uuid::new_v4().to_string(),
            true,
            DEFAULT_RECV_MAX,
            true,
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
    ) -> Self {
        let mut client = Self::new(client_id, auto_ack, receive_max, auto_packet_id);
        client.connection = Some(connection);
        client
    }

    pub(crate) fn new(
        client_id: &str,
        auto_ack: bool,
        receive_max: u16,
        auto_packet_id: bool,
    ) -> Self {
        Self {
            connection: None,
            auto_ack,
            auto_packet_id,
            receive_max,
            connected: Arc::new(RwLock::new(false)),
            session_expiry: DEFAULT_SESSION_EXPIRY,
            client_id: Arc::new(Mutex::new(Some(client_id.to_string()))),
            filter_channel: HashMap::new(),
            packet_in: None,
            packet_out: None,
            err_chan: None,
            pending_qos1: Arc::new(Mutex::new(Vec::new())),
            max_packet_size: DEFAULT_MAX_PACKET_SIZE,
            keep_alive: DEFAULT_CLIENT_KEEP_ALIVE,
            max_connect_wait: MAX_CONNECT_WAIT,
        }
    }

    pub(crate) fn set_packet_in(&mut self, packet_in: PacketChannel) {
        self.packet_in = Some(packet_in);
    }

    pub(crate) fn set_packet_out(&mut self, packet_out: Sender<vaux_mqtt::Packet>) {
        self.packet_out = Some(packet_out);
    }

    pub(crate) fn set_max_packet_size(&mut self, max_packet_size: usize) {
        self.max_packet_size = max_packet_size;
    }

    pub async fn connected(&self) -> bool {
        *self.connected.read().await
    }

    pub fn session_expiry(&self) -> u32 {
        self.session_expiry
    }

    pub(crate) fn set_session_expiry(&mut self, session_expiry: u32) {
        self.session_expiry = session_expiry;
    }

    pub(crate) fn set_error_out(&mut self, error_out: Sender<MqttError>) {
        self.err_chan = Some(error_out);
    }

    pub(crate) fn set_keep_alive(&mut self, keep_alive: Duration) {
        if keep_alive < MIN_KEEP_ALIVE {
            self.keep_alive = MIN_KEEP_ALIVE;
        } else {
            self.keep_alive = keep_alive;
        }
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
        if let Some(packet_in_sender) = self.packet_in.as_ref() {
            packet_in_sender
                .0
                .send(vaux_mqtt::Packet::Subscribe(subscribe))
                .await
        } else {
            Err(SendError(vaux_mqtt::Packet::Subscribe(subscribe)))
        }
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
    ///
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let conn = MqttConnection::new().with_host("localhost").with_port(1883);
    ///     let mut client = MqttClient::default();
    ///     client.set_connection(conn);
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
        let keep_alive = self.keep_alive;

        let mut packet_in_receiver = self.packet_in.as_mut().unwrap().1.take().unwrap();
        let packet_out = self.packet_out.clone().unwrap();
        let credentials = self.connection.as_ref().unwrap().credentials().clone();
        let connected = self.connected.clone();
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
        let mut session = match ClientSession::try_from(self) {
            Ok(s) => s,
            Err(e) => {
                return Err(MqttError::new(
                    &format!("unable to create client session: {}", e),
                    ErrorKind::Transport,
                ));
            }
        };
        Ok(tokio::spawn(async move {
            match session
                .connect(max_connect_wait, credentials, clean_start)
                .await
            {
                Ok(_) => *connected.write().await = true,
                Err(e) => {
                    return Err(e);
                }
            }
            loop {
                select! {
                    _ = MqttClient::keep_alive_timer(keep_alive) => {
                        match session.keep_alive().await {
                            Ok(_) => {
                                // do nothing
                            }
                            Err(e) => {
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

                                        packet_out.send(packet).await.map_err(|e| {
                                            MqttError::new(
                                                &format!("unable to send packet: {}", e),
                                                ErrorKind::Transport,
                                            )
                                        })?;
                                    }
                                    Ok(None) => {
                                        // do nothing
                                    }
                                    Err(e) => {
                                        return Err(e);
                                    }
                                }
                            }
                            Ok(None) => {
                                // do nothing
                            }
                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                    pending_packet = packet_in_receiver.recv() => {
                        match pending_packet {
                            Some(inbound_packet) => {
                                match session.write_next(inbound_packet).await {
                                    Ok(_) => {
                                    }
                                    Err(e) => {
                                        return Err(e);
                                    }
                                }
                            }
                            None => {
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

    async fn keep_alive_timer(keep_alive: Duration) {
        tokio::time::sleep(keep_alive).await;
    }

    pub async fn stop(&mut self) -> Result<(), MqttError> {
        let disconnect = Packet::Disconnect(Default::default());
        if let Some(packet_in) = self.packet_in.as_ref() {
            if let Err(e) = packet_in.0.send(disconnect).await {
                return Err(MqttError::new(
                    &format!("unable to send disconnect: {}", e),
                    ErrorKind::Transport,
                ));
            }
        } else {
            return Err(MqttError::new(
                "unable to send disconnect, no packet out channel",
                ErrorKind::Transport,
            ));
        }
        Ok(())
    }

    // pub async fn send(
    //     connection: &mut AsyncMqttStream,
    //     send_timeout: Duration,
    //     packet: Packet,
    // ) -> crate::Result<Option<Packet>> {
    //     let mut dest = BytesMut::default();
    //     let result = encode(&packet, &mut dest);
    //     if let Err(e) = result {
    //         panic!("Failed to encode packet: {:?}", e);
    //     }
    //     match tokio::time::timeout(send_timeout, connection.write_all(&dest)).await {
    //         Ok(result) => match result {
    //             Ok(_) => Ok(None),
    //             Err(e) => {
    //                 return Err(MqttError::new(
    //                     &format!("unable to send packet: {}", e),
    //                     ErrorKind::IO,
    //                 ));
    //             }
    //         },
    //         Err(e) => {
    //             return Err(MqttError::new(
    //                 &format!("unable to send packet: {}", e),
    //                 ErrorKind::Timeout,
    //             ));
    //         }
    //     }
    // }
}
