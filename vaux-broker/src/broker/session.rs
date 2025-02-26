use std::{
    collections::HashMap,
    sync::{atomic::AtomicU32, Arc},
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use vaux_mqtt::{subscribe::Subscription, Connect, WillMessage};

const BROKER_KEEP_ALIVE_FACTOR: f32 = 1.5;

#[derive(Debug, Default)]
pub struct SessionPool {
    active: HashMap<String, Arc<RwLock<Session>>>,
    inactive: HashMap<String, Arc<RwLock<Session>>>,
    active_sessions: AtomicU32,
    inactive_sessions: AtomicU32,
}

impl SessionPool {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn active_sessions(&self) -> u32 {
        self.active_sessions
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn inactive_sessions(&self) -> u32 {
        self.inactive_sessions
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Inserts a new session into the pool. If the session is connected, the
    /// active session count is incremented and the session is added to the active
    /// session pool. If the session is not connected, the inactive session count
    /// is incremented and the session is added to the inactive session pool.
    pub async fn add(&mut self, session: &Arc<RwLock<Session>>) {
        let active;
        {
            let session = session.read().await;
            active = session.connected();
        }
        if active {
            self.active_sessions
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.active
                .insert(session.read().await.id().to_string(), Arc::clone(session));
        } else {
            self.inactive_sessions
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.inactive
                .insert(session.read().await.id().to_string(), Arc::clone(session));
        }
    }

    /// Removes a session from the pool. The active and inactive counts are decremented
    /// as required and the session is removed from the appropriate pool.
    ///
    pub fn remove(&mut self, session_id: &str) -> Option<Arc<RwLock<Session>>> {
        if let Some(session) = self.active.remove(session_id) {
            self.active_sessions
                .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            Some(session)
        } else if let Some(session) = self.inactive.remove(session_id) {
            self.inactive_sessions
                .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            Some(session)
        } else {
            None
        }
    }

    /// Removes an active session from the pool. The session is removed from the active
    /// pool and the active session count is decremented. The session is returned if found.
    /// If the session is not found, the function returns None.
    ///
    pub fn remove_active(&mut self, session_id: &str) -> Option<Arc<RwLock<Session>>> {
        if let Some(session) = self.active.remove(session_id) {
            self.active_sessions
                .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            Some(session)
        } else {
            None
        }
    }

    /// Retreives and activates a session if the inactive session pool contains the session.
    /// The session is removed from the inactive pool and added to the active pool. The active
    /// session count is incremented and the inactive session count is decremented.  If the
    /// session is not found in the inactive pool, the function returns None.
    ///
    /// The session `connected` flag is set to true and the last active time is set to the current
    /// time if the session is found.
    pub async fn activate(&mut self, session_id: &str) -> Option<Arc<RwLock<Session>>> {
        if let Some(session_guard) = self.inactive.remove(session_id) {
            let mut session = session_guard.write().await;
            session.set_connected(true);
            session.set_last_active();
            self.active_sessions
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.active
                .insert(session.id().to_string(), Arc::clone(&session_guard));
            self.inactive_sessions
                .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            drop(session);
            Some(session_guard)
        } else {
            None
        }
    }
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
    pub fn new(id: String, keep_alive: Duration) -> Self {
        Session {
            id: id,
            last_active: Instant::now(),
            connected: false,
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

#[cfg(test)]
pub mod test {
    use vaux_mqtt::property::Property;

    use super::*;
    use std::time::Duration;

    #[test]
    fn test_session_new() {
        let session = Session::new("test".to_string(), Duration::from_secs(10));
        assert_eq!(session.id(), "test");
        assert_eq!(session.connected(), false);
        assert_eq!(session.orphaned(), false);
        assert_eq!(session.keep_alive(), Duration::from_secs(10));
        assert_eq!(session.subscriptions.len(), 0);
        assert_eq!(session.will_message(), None);
        assert_eq!(session.session_expiry, Duration::new(0, 0));
    }

    #[test]
    fn test_session_new_from_connect() {
        const TEST_KEEP_ALIVE: u16 = 55;
        const TEST_EXPIRY: u32 = 33;

        let mut connect = Connect::default();
        connect.client_id = "test".to_string();
        connect.keep_alive = TEST_KEEP_ALIVE;
        connect.will_message = Some(WillMessage::new(vaux_mqtt::QoSLevel::AtLeastOnce, true));
        connect
            .properties_mut()
            .set_property(Property::SessionExpiryInterval(TEST_EXPIRY));

        let session = Session::new_from_connect(connect);
        assert_eq!(session.id(), "test");
        assert_eq!(session.connected(), true);
        assert_eq!(session.orphaned(), false);
        assert_eq!(
            session.keep_alive(),
            Duration::from_secs((TEST_KEEP_ALIVE as f32 * BROKER_KEEP_ALIVE_FACTOR) as u64)
        );
        assert_eq!(session.subscriptions.len(), 0);
        assert_eq!(
            session.will_message(),
            Some(&WillMessage::new(vaux_mqtt::QoSLevel::AtLeastOnce, true))
        );
        assert_eq!(
            session.session_expiry,
            Duration::from_secs(TEST_EXPIRY as u64)
        );
    }

    #[tokio::test]
    async fn test_add_active() {
        let mut pool = SessionPool::new();
        let mut session = Session::new("test".to_string(), Duration::from_secs(10));
        session.set_connected(true);
        let session = Arc::new(RwLock::new(session));
        pool.add(&session).await;
        assert_eq!(pool.active_sessions(), 1);
        assert_eq!(pool.inactive_sessions(), 0);
    }

    #[tokio::test]
    async fn test_session_activate() {
        let mut pool = SessionPool::new();
        let session = Arc::new(RwLock::new(Session::new(
            "test_1".to_string(),
            Duration::from_secs(10),
        )));
        pool.add(&session).await;
        assert_eq!(pool.active_sessions(), 0);
        assert_eq!(pool.inactive_sessions(), 1);
        pool.activate("test_1").await;
        assert_eq!(pool.active_sessions(), 1);
        assert_eq!(pool.inactive_sessions(), 0);
        // get the session and validate connection and last active
        let session = pool.active.get("test_1").unwrap();
        let session = session.read().await;
        assert_eq!(session.connected(), true);
        assert!(session.last_active.elapsed() < Duration::from_millis(500));
    }
}
