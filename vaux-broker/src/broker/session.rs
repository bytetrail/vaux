use std::time::{Duration, Instant};
use vaux_mqtt::{subscribe::Subscription, Connect, WillMessage};

const BROKER_KEEP_ALIVE_FACTOR: f32 = 1.5;

pub enum SessionControl {
    Disconnect,
    TakenOver,
    KeepAlive,
}

#[derive(Debug)]
pub struct Session {
    id: String,
    last_active: Instant,
    connected: bool,
    orphaned: bool,
    keep_alive: Duration,
    pub session_expiry: Duration,
    will_message: Option<WillMessage>,
    subscriptions: Vec<Subscription>,
}

impl Session {
    /// Creates a new session with the last active time set to Instant::now()
    pub fn new(id: String, stream: vaux_async::stream::PacketStream, keep_alive: Duration) -> Self {
        Session {
            id: id,
            last_active: Instant::now(),
            connected: true,
            orphaned: false,
            keep_alive,
            session_expiry: Duration::new(0, 0),
            will_message: None,
            subscriptions: Vec::new(),
        }
    }

    pub fn new_from_connect(connect: Connect) -> Self {
        let keep_alive =
            Duration::from_secs((connect.keep_alive as f32 * BROKER_KEEP_ALIVE_FACTOR) as u64);
        let will_message = connect.will_message.clone();
        let session_expiry = if let Some(session_expirey) = connect
            .properties()
            .get_property(vaux_mqtt::PropertyType::SessionExpiryInterval)
        {
            match session_expirey {
                vaux_mqtt::property::Property::SessionExpiryInterval(se) => {
                    Duration::from_secs(*se as u64)
                }
                _ => Duration::new(0, 0),
            }
        } else {
            Duration::new(0, 0)
        };

        Session {
            id: connect.client_id.to_string(),
            last_active: Instant::now(),
            connected: true,
            orphaned: false,
            keep_alive,
            session_expiry,
            will_message,
            subscriptions: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.subscriptions.clear();
        // TODO - remove the subscriptions from the broker subscription pool
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn connected(&self) -> bool {
        self.connected
    }

    pub(crate) fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }

    pub fn orphaned(&self) -> bool {
        self.orphaned
    }

    pub(crate) fn set_orphaned(&mut self) {
        self.orphaned = true;
    }

    /// Sets the last session activity to the time that the method is invoked.
    pub(crate) fn set_last_active(&mut self) {
        self.last_active = Instant::now();
    }

    /// Gets the maximum keep alive in seconds.
    pub fn keep_alive(&self) -> Duration {
        self.keep_alive
    }
    /// Sets the maximum session keep alive to the time passed in seconds.
    pub(crate) fn set_keep_alive(&mut self, keep_alive: Duration) {
        self.keep_alive = keep_alive;
    }

    pub fn will_message(&self) -> Option<&WillMessage> {
        self.will_message.as_ref()
    }
}
