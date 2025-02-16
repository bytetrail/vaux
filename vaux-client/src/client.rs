use crate::{stream::AsyncMqttStream, ErrorKind, MqttConnection, MqttError};
use bytes::BytesMut;
use std::{collections::HashMap, sync::Arc, time::Duration, vec};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{
    sync::{
        mpsc::{self, error::SendError, Receiver, Sender},
        Mutex,
    },
    task::JoinHandle,
};
use vaux_mqtt::{
    decode, encode, property::Property, ConnAck, Connect, Packet, PacketType, PropertyType,
    PubResp, QoSLevel, Reason, Subscribe, SubscriptionFilter,
};

const DEFAULT_RECV_MAX: u16 = 100;
const DEFAULT_SESSION_EXPIRY: u32 = 1000;
// 64K is the default max packet size
const DEFAULT_MAX_PACKET_SIZE: usize = 64 * 1024;
const MAX_QUEUE_LEN: usize = 100;
const DEFAULT_CLIENT_KEEP_ALIVE: Duration = Duration::from_secs(60);
const MIN_KEEP_ALIVE: Duration = Duration::from_secs(30);
const MAX_CONNECT_WAIT: Duration = Duration::from_secs(5);
const DEFAULT_CHANNEL_SIZE: usize = 128;

type FilteredChannel = HashMap<PacketType, Sender<vaux_mqtt::Packet>>;

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
    connection: Option<MqttConnection>,
    auto_ack: bool,
    receive_max: u16,
    auto_packet_id: bool,
    last_packet_id: u16,
    filter_channel: FilteredChannel,
    connected: Arc<Mutex<bool>>,
    last_error: Arc<Mutex<Option<MqttError>>>,
    session_expiry: u32,
    client_id: Arc<Mutex<Option<String>>>,
    packet_in: Option<Receiver<vaux_mqtt::Packet>>,
    packet_out: Option<Sender<vaux_mqtt::Packet>>,
    err_chan: Option<Sender<MqttError>>,
    subscriptions: Vec<SubscriptionFilter>,
    pending_qos1: Arc<Mutex<Vec<Packet>>>,
    max_packet_size: usize,
    keep_alive: Duration,
    max_connect_wait: Duration,
    send_timeout: Duration,
    receive_timeout: Duration,
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

    /// Creates a new MQTT client with the specified host, port, client ID, and
    /// auto ack settings. The client ID is required and must be unique for the
    /// broker. If the client ID is not specified, a UUID will be generated and
    /// used as the client ID.
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
            last_packet_id: 0,
            last_error: Arc::new(Mutex::new(None)),
            receive_max,
            connected: Arc::new(Mutex::new(false)),
            session_expiry: DEFAULT_SESSION_EXPIRY,
            client_id: Arc::new(Mutex::new(Some(client_id.to_string()))),
            filter_channel: HashMap::new(),
            packet_in: None,
            packet_out: None,
            err_chan: None,
            subscriptions: Vec::new(),
            pending_qos1: Arc::new(Mutex::new(Vec::new())),
            max_packet_size: DEFAULT_MAX_PACKET_SIZE,
            keep_alive: DEFAULT_CLIENT_KEEP_ALIVE,
            max_connect_wait: MAX_CONNECT_WAIT,
            send_timeout: Duration::from_secs(5),
            receive_timeout: Duration::from_secs(5),
        }
    }

    pub(crate) fn set_packet_in(&mut self, packet_in: Receiver<vaux_mqtt::Packet>) {
        self.packet_in = Some(packet_in);
    }

    pub(crate) fn set_packet_out(&mut self, packet_out: Sender<vaux_mqtt::Packet>) {
        self.packet_out = Some(packet_out);
    }

    pub fn max_packet_size(&self) -> usize {
        self.max_packet_size
    }

    pub(crate) fn set_max_packet_size(&mut self, max_packet_size: usize) {
        self.max_packet_size = max_packet_size;
    }

    pub async fn connected(&self) -> bool {
        *self.connected.lock().await
    }

    pub fn session_expiry(&self) -> u32 {
        self.session_expiry
    }

    /// Sets the session expiry for the client. The session expiry is the number
    /// of seconds that the broker will maintain the session for the client after
    /// the client disconnects. If the client reconnects within the session expiry
    /// interval, the broker will resume the session. If the client does not
    /// reconnect within the session expiry interval, the broker will discard the
    /// session and any state associated with the session. The session_expiry must
    /// be set prior to calling connect for the value to be used.
    ///
    /// The default session expiry is 0 seconds, so no session information would be
    /// stored by the broker with the default set.
    /// Example:
    /// ```
    /// use vaux_client::MqttClient;
    ///
    /// let mut client = MqttClient::default();
    /// // set the session expiry to 1 day
    /// client.set_session_expiry(60 * 60 * 24);
    /// ```
    pub(crate) fn set_session_expiry(&mut self, session_expiry: u32) {
        self.session_expiry = session_expiry;
    }

    /// Sets up an error handler for the client. The error handler is used to receive
    /// errors from the client. If the error handler is set, the client will send
    /// errors to the error handler. The error handler is a channel that is used to
    /// receive errors from the client. Only one error handler can be set at a time.
    ///
    /// Errors that occur on the client once a connection has been established will be
    /// sent to the error handler. This includes transport, protocol, and codec errors.
    /// PUBACK failures will be sent to the error handler if the client does not receive
    /// a PUBACK for a QoS 1 message within the specified time or the PUBACK has a reason
    /// code other than "Success".
    ///
    /// It is not recommended to use the automatic packet ID feature with an error handler
    /// as the receiver of the error will not be able to determine which packet caused the
    /// error if the packet ID is automatically generated.
    ///
    /// The error handler as with other configuration settings must be set prior to calling
    /// start for the error handler to be used.
    pub(crate) fn set_error_out(&mut self, error_out: Sender<MqttError>) {
        self.err_chan = Some(error_out);
    }

    /// Sets the keep alive interval for the client. The keep alive interval is
    /// the number of seconds that the client will send a PING packet to the
    /// broker to keep the connection alive. If the client does not receive a
    /// PINGRESP from the broker within the keep alive interval the client will
    /// disconnect from the broker.
    ///
    /// The keep alive interval must be set prior to calling connect for the value
    /// to be used.
    ///
    /// The default keep alive interval is 60 seconds. The minimum keep alive
    /// interval is 30 seconds. If a keep alive interval less than 30 seconds is
    /// set, the client will use the minimum keep alive interval of 30 seconds.
    ///
    /// Example:
    /// ```
    /// use vaux_client::MqttClient;
    ///
    /// let mut client = MqttClient::default();
    /// // set the keep alive interval to 30 seconds
    /// client.set_keep_alive(30);
    /// ```
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

    /// Sets the maximum connection wait time. This is the maximum time that
    /// the client will wait for a connection to be established with the
    /// remote broker before returning an error. This value must be set prior
    /// to calling try_start for the value to have any effect.
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
            self.subscriptions.push(subscription.clone());
            subscribe.add_subscription(subscription);
        }
        if let Some(packet_out) = self.packet_out.as_ref() {
            packet_out
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
        let start = std::time::Instant::now();
        while !self.connected().await {
            if let Ok(last_error_guard) = self.last_error.try_lock() {
                let last_error = last_error_guard.clone();
                drop(last_error_guard);
                if let Some(last_error) = last_error {
                    match handle.await {
                        Ok(result) => {
                            result?;
                        }
                        Err(e) => {
                            return Err(MqttError::new(
                                &format!("unable to join thread: {:?}", e),
                                ErrorKind::Transport,
                            ));
                        }
                    }
                    return Err(last_error);
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if start.elapsed() > max_wait {
                return Err(MqttError::new(
                    "timeout waiting for connection",
                    ErrorKind::Timeout,
                ));
            }
        }
        Ok(handle)
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
    pub async fn start(&mut self, clean_start: bool) -> JoinHandle<crate::Result<()>> {
        //let packet_recv = self.packet_recv.as_ref().unwrap().clone();
        //let mut packet_send = self.packet_send.take().unwrap();
        let auto_ack = self.auto_ack;
        let receive_max = self.receive_max;
        let pending_qos1 = self.pending_qos1.clone();
        let mut last_packet_id = self.last_packet_id;
        let auto_packet_id = self.auto_packet_id;
        let max_packet_size = self.max_packet_size;
        let client_id = self.client_id.clone();
        let session_expiry = self.session_expiry;
        let connected = self.connected.clone();
        let credentials = self.connection.as_ref().and_then(|c| c.credentials());
        let last_error = self.last_error.clone();
        let err_chan = self.err_chan.as_ref().map(|e| e.clone());
        let keep_alive = self.keep_alive;
        let filter_channel = self.filter_channel.clone();
        let max_connect_wait = self.max_connect_wait;
        let connection = self.connection.take();
        let mut packet_in = self.packet_in.take().unwrap();
        let packet_out = self.packet_out.as_ref().unwrap().clone();
        let send_timeout = self.send_timeout;
        let receive_timeout = self.receive_timeout;

        tokio::spawn(async move {
            if connection.is_none() {
                return Err(MqttError::new("connection not set", ErrorKind::Connection));
            }
            let connection = connection.unwrap();
            let mut buffer = vec![0; max_packet_size];
            let mut offset = 0;

            let mut stream = match connection.connect().await {
                Ok(c) => c,
                Err(e) => {
                    return Err(MqttError::new(
                        &format!("unable to connect to broker: {}", e),
                        ErrorKind::Transport,
                    ));
                }
            };

            match Self::connect(
                credentials,
                client_id,
                session_expiry,
                clean_start,
                connected,
                &mut buffer,
                &mut offset,
                max_connect_wait,
                &mut stream,
            )
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    let last_error = last_error.lock().await.clone();
                    //                    stream.shutdown().unwrap();
                    return Err(last_error.unwrap_or(e));
                }
            }
            let mut pending_recv_ack: HashMap<u16, Packet> = HashMap::new();
            let mut pending_publish: Vec<Packet> = Vec::new();
            // TODO add size tracking to pending publish
            // let mut pending_publish_size = 0;
            let mut qos_1_remaining = receive_max;
            pending_publish.append(&mut *pending_qos1.lock().await);
            let mut last_active = std::time::Instant::now();
            loop {
                match MqttClient::read_next(
                    &mut stream,
                    max_packet_size,
                    &mut buffer,
                    &mut offset,
                    receive_timeout,
                )
                .await
                {
                    Ok(result) => {
                        if let Some(p) = result {
                            let mut packet_to_consumer = true;
                            match &p {
                                Packet::PingResponse(_pingresp) => {
                                    // do not send to consumer
                                    packet_to_consumer = false;
                                }
                                Packet::Disconnect(d) => {
                                    // TODO handle disconnect - verify shutdown behavior
                                    let _ = stream.shutdown().await;
                                    pending_qos1.lock().await.append(&mut pending_publish);
                                    return Err(MqttError::new(
                                        &format!("disconnect received: {:?}", d),
                                        ErrorKind::Protocol(d.reason),
                                    ));
                                }
                                Packet::Publish(publish) => {
                                    match publish.qos() {
                                        vaux_mqtt::QoSLevel::AtMostOnce => {}
                                        vaux_mqtt::QoSLevel::AtLeastOnce => {
                                            if auto_ack {
                                                let mut puback = PubResp::new_puback();
                                                if let Some(packet_id) = publish.packet_id {
                                                    puback.packet_id = packet_id;
                                                } else {
                                                    let _ = stream.shutdown().await;
                                                    return Err(MqttError::new(
                                                        "protocol error, packet ID required with QoS > 0",
                                                        ErrorKind::Protocol(
                                                            Reason::MalformedPacket,
                                                      ),
                                                    ));
                                                }
                                                if MqttClient::send(
                                                    &mut stream,
                                                    send_timeout,
                                                    Packet::PubAck(puback),
                                                )
                                                .await
                                                .is_err()
                                                {
                                                    // TODO handle the pub ack next time through
                                                    // push a message to the last error channel\
                                                    todo!()
                                                }
                                            }
                                        }
                                        vaux_mqtt::QoSLevel::ExactlyOnce => todo!(),
                                    }
                                }
                                Packet::PubAck(puback) => {
                                    if let Some(_p) = pending_recv_ack.remove(&puback.packet_id) {
                                        if qos_1_remaining < receive_max {
                                            qos_1_remaining += 1;
                                        }
                                    } else {
                                        // TODO PUBACK that was not expected
                                    }
                                }
                                _ => {}
                            }
                            if packet_to_consumer {
                                if let Some(sender) = filter_channel.get(&PacketType::from(&p)) {
                                    if let Err(e) = sender.send(p.clone()).await {
                                        return Err(MqttError::new(
                                            &format!("unable to send packet to consumer: {}", e),
                                            ErrorKind::Transport,
                                        ));
                                    }
                                } else {
                                    // no filter for packet type, send on the general channel
                                    if let Err(e) = packet_out.try_send(p.clone()) {
                                        let _ = stream.shutdown().await;
                                        pending_qos1.lock().await.append(&mut pending_publish);
                                        return Err(MqttError::new(
                                            &format!("unable to send packet to consumer: {}", e),
                                            ErrorKind::Transport,
                                        ));
                                    }
                                }
                            }
                        }
                    }

                    Err(e) => {
                        if e.kind() != ErrorKind::Timeout {
                            // TODO evaluate additional error types
                            if let Some(chan) = err_chan.as_ref() {
                                if chan.try_send(e.clone()).is_err() {
                                    return Err(e);
                                }
                            } else {
                                return Err(e);
                            }
                        }
                    }
                };
                if let Ok(mut packet) = packet_in.try_recv() {
                    if let Packet::Publish(mut p) = packet.clone() {
                        if p.qos() == QoSLevel::AtLeastOnce {
                            if auto_packet_id && p.packet_id.is_none() {
                                last_packet_id += 1;
                                p.packet_id = Some(last_packet_id);
                                pending_recv_ack.insert(last_packet_id, Packet::Publish(p.clone()));
                            } else if let Some(packet_id) = p.packet_id {
                                pending_recv_ack.insert(packet_id, Packet::Publish(p.clone()));
                            } else {
                                let err = MqttError::new(
                                    "no packet ID for QoS > 0",
                                    ErrorKind::Protocol(Reason::MalformedPacket),
                                );
                                if let Some(chan) = err_chan.as_ref() {
                                    if chan.send(err.clone()).await.is_err() {
                                        return Err(err);
                                    }
                                } else {
                                    return Err(err);
                                }
                            }
                            // if we have additional capacity for QOS 1 PUBACK
                            if qos_1_remaining > 0 {
                                qos_1_remaining -= 1;
                                packet = Packet::Publish(p);
                            } else {
                                // TODO cannot send the packet - need to inform client
                                if pending_publish.len() < MAX_QUEUE_LEN {
                                    // && pending_publish_size < MAX_QUEUE_SIZE {
                                    pending_publish.push(Packet::Publish(p));
                                    continue;
                                }
                            }
                        }
                    } else if let Packet::Disconnect(_d) = packet.clone() {
                        if let Err(e) = MqttClient::send(&mut stream, send_timeout, packet).await {
                            if let Some(chan) = err_chan.as_ref() {
                                if chan.send(e.clone()).await.is_err() {
                                    return Err(e);
                                }
                            } else {
                                return Err(e);
                            }
                        }
                        // TODO handle shutdown error?
                        let _ = stream.shutdown().await;
                        pending_qos1.lock().await.append(&mut pending_publish);
                        return Ok(());
                    }
                    if let Err(e) = MqttClient::send(&mut stream, send_timeout, packet).await {
                        if let Some(chan) = err_chan.as_ref() {
                            if chan.send(e.clone()).await.is_err() {
                                return Err(e);
                            }
                        } else {
                            return Err(e);
                        }
                    }
                    // packet sent, update last active time
                    last_active = std::time::Instant::now();
                }
                if last_active.elapsed() > keep_alive {
                    // use idle time to attempt to resend any pending QOS-1 packets
                    if !pending_publish.is_empty() && qos_1_remaining > 0 {
                        // send any pending QOS-1 publish packets that we are able to send
                        while !pending_publish.is_empty() && qos_1_remaining > 0 {
                            while !pending_publish.is_empty() && qos_1_remaining > 0 {
                                let packet = pending_publish.remove(0);
                                if let Err(e) =
                                    MqttClient::send(&mut stream, send_timeout, packet.clone())
                                        .await
                                {
                                    pending_publish.insert(0, packet);
                                    if let Some(chan) = err_chan.as_ref() {
                                        if chan.send(e.clone()).await.is_err() {
                                            return Err(e);
                                        }
                                    } else {
                                        return Err(e);
                                    }
                                } else {
                                    qos_1_remaining += 1;
                                }
                            }
                        }
                        // packet sent, update last active time
                        last_active = std::time::Instant::now();
                    } else {
                        let ping = Packet::PingRequest(Default::default());
                        if let Err(e) = MqttClient::send(&mut stream, send_timeout, ping).await {
                            if let Some(chan) = err_chan.as_ref() {
                                if chan.send(e.clone()).await.is_err() {
                                    return Err(e);
                                }
                            } else {
                                return Err(e);
                            }
                        }
                        // packet sent, update last active time
                        last_active = std::time::Instant::now();
                    }
                }
            }
        })
    }

    pub async fn stop(&mut self) -> Result<(), MqttError> {
        let disconnect = Packet::Disconnect(Default::default());
        if let Some(packet_out) = self.packet_out.as_ref() {
            if let Err(e) = packet_out.send(disconnect).await {
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

    #[allow(clippy::too_many_arguments)]
    async fn connect<'a>(
        credentials: Option<(String, String)>,
        client_id: Arc<Mutex<Option<String>>>,
        session_expiry: u32,
        clean_start: bool,
        connected: Arc<Mutex<bool>>,
        buffer: &mut [u8],
        offset: &mut usize,
        max_connect_wait: Duration,
        stream: &mut AsyncMqttStream,
    ) -> crate::Result<ConnAck> {
        let mut connect = Connect::default();
        connect.clean_start = clean_start;
        {
            let set_id = client_id.lock().await;
            if set_id.is_some() {
                connect.client_id = (*set_id.as_ref().unwrap()).to_string();
            }
        }
        connect
            .properties_mut()
            .set_property(Property::SessionExpiryInterval(session_expiry));
        if let Some((username, password)) = credentials {
            connect.username = Some(username);
            connect.password = Some(password.into_bytes());
        }
        let connect_packet = Packet::Connect(Box::new(connect));
        // let mut buffer = [0u8; DEFAULT_CHANNEL_SIZE];
        let mut dest = BytesMut::default();
        let result = encode(&connect_packet, &mut dest);
        if let Err(e) = result {
            panic!("Failed to encode packet: {:?}", e);
        }

        match stream.write_all(&dest).await {
            Ok(_) => {
                let start = std::time::Instant::now();
                while start.elapsed() < max_connect_wait {
                    match MqttClient::read_next(
                        stream,
                        DEFAULT_MAX_PACKET_SIZE,
                        buffer,
                        offset,
                        max_connect_wait,
                    )
                    .await
                    {
                        Ok(Some(packet)) => match packet {
                            Packet::ConnAck(connack) => {
                                return Self::handle_connack(connack, connected, client_id).await;
                            }
                            Packet::Disconnect(disconnect) => {
                                return Err(MqttError::new(
                                    &format!("disconnect received: {}", disconnect.reason),
                                    ErrorKind::Protocol(disconnect.reason),
                                ))
                            }
                            _ => {
                                return Err(MqttError::new(
                                    "unexpected packet type",
                                    ErrorKind::Protocol(Reason::ProtocolErr),
                                ))
                            }
                        },
                        Ok(None) => {
                            return Err(MqttError::new(
                                "no MQTT packet received",
                                ErrorKind::Protocol(Reason::ProtocolErr),
                            ))
                        }
                        Err(e) => match e.kind {
                            ErrorKind::Timeout => {
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                            _ => {
                                return Err(MqttError::new(
                                    &format!("unable to read packet: {}", e),
                                    ErrorKind::Transport,
                                ));
                            }
                        },
                    }
                }
                Err(MqttError::new(
                    "unable to connect to broker",
                    ErrorKind::Timeout,
                ))
            }
            Err(e) => Err(MqttError::new(
                &format!("Unable to write packet(s) to broker: {}", e),
                ErrorKind::Transport,
            )),
        }
    }

    async fn handle_connack(
        connack: ConnAck,
        connected: Arc<Mutex<bool>>,
        client_id: Arc<Mutex<Option<String>>>,
    ) -> crate::Result<ConnAck> {
        let set_id = client_id.lock().await;
        let client_id_set = set_id.is_some();
        if connack.reason() != Reason::Success {
            // TODO return the connack reason as MQTT error with reason code
            let mut connected = connected.lock().await;
            *connected = false;
            return Err(MqttError::new(
                "connection refused",
                ErrorKind::Protocol(connack.reason()),
            ));
        } else {
            let mut connected = connected.lock().await;
            *connected = true;
        }
        if !client_id_set {
            match connack
                .properties()
                .get_property(PropertyType::AssignedClientId)
            {
                Some(Property::AssignedClientId(id)) => {
                    let mut client_id = client_id.lock().await;
                    *client_id = Some(id.to_owned());
                }
                _ => {
                    // handle error here for required property
                    Err(MqttError::new(
                        "no assigned client id",
                        ErrorKind::Protocol(Reason::InvalidClientId),
                    ))?;
                }
            }
        }
        // TODO set server properties based on ConnAck
        Ok(connack)
    }

    async fn read_next(
        connection: &mut AsyncMqttStream,
        max_packet_size: usize,
        buffer: &mut [u8],
        offset: &mut usize,
        timeout: Duration,
    ) -> crate::Result<Option<Packet>> {
        let mut bytes_read = *offset;
        loop {
            if bytes_read > 0 {
                let bytes_mut = &mut BytesMut::from(&buffer[0..bytes_read]);
                match decode(bytes_mut) {
                    Ok(data_read) => {
                        if let Some((packet, decode_len)) = data_read {
                            if decode_len < bytes_read as u32 {
                                buffer.copy_within(decode_len as usize..bytes_read, 0);
                                // adjust offset to end of decoded bytes
                                *offset = bytes_read - decode_len as usize;
                            } else {
                                *offset = 0;
                            }
                            return Ok(Some(packet));
                        } else {
                            return Ok(None);
                        }
                    }
                    Err(e) => match e.kind {
                        vaux_mqtt::codec::ErrorKind::InsufficientData(_expected, _actual) => {
                            // fall through the the socket read
                        }
                        _ => {
                            return Err(MqttError::new(
                                &e.to_string(),
                                crate::ErrorKind::Protocol(Reason::ProtocolErr),
                            ));
                        }
                    },
                }
            }
            match tokio::time::timeout(timeout, {
                connection.read(&mut buffer[*offset..max_packet_size])
            })
            .await
            {
                Ok(result) => match result {
                    Ok(len) => {
                        if len == 0 && bytes_read == 0 {
                            return Ok(None);
                        }
                        bytes_read += len;
                        *offset = bytes_read;
                    }
                    Err(e) => match e.kind() {
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => {
                            return Err(MqttError::new(&e.to_string(), ErrorKind::Timeout));
                        }
                        e => {
                            return Err(MqttError::new(&e.to_string(), ErrorKind::IO));
                        }
                    },
                },
                Err(e) => {
                    return Err(MqttError::new(&e.to_string(), ErrorKind::Timeout));
                }
            }
        }
    }

    pub async fn send(
        connection: &mut AsyncMqttStream,
        send_timeout: Duration,
        packet: Packet,
    ) -> crate::Result<Option<Packet>> {
        let mut dest = BytesMut::default();
        let result = encode(&packet, &mut dest);
        if let Err(e) = result {
            panic!("Failed to encode packet: {:?}", e);
        }
        match tokio::time::timeout(send_timeout, connection.write_all(&dest)).await {
            Ok(result) => match result {
                Ok(_) => Ok(None),
                Err(e) => {
                    return Err(MqttError::new(
                        &format!("unable to send packet: {}", e),
                        ErrorKind::IO,
                    ));
                }
            },
            Err(e) => {
                return Err(MqttError::new(
                    &format!("unable to send packet: {}", e),
                    ErrorKind::Timeout,
                ));
            }
        }
    }
}
