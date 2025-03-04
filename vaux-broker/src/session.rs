use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{
    mpsc::{self, Receiver, Sender},
    RwLock,
};
use vaux_mqtt::{subscribe::Subscription, Reason, WillMessage};

#[derive(Debug, Default)]
pub struct SessionPool {
    session_expiry: Duration,
    default_keep_alive: Duration,
    max_keep_alive: Duration,
    active: HashMap<String, Arc<RwLock<Session>>>,
    inactive: HashMap<String, Arc<RwLock<Session>>>,
    active_sessions: u32,
    inactive_sessions: u32,
}

impl SessionPool {
    pub fn new_with_expiry(expiry: Duration) -> Self {
        SessionPool {
            session_expiry: expiry,
            ..Default::default()
        }
    }

    pub fn new_with_config(config: &crate::config::Config) -> Self {
        SessionPool {
            session_expiry: config.session_expiry,
            default_keep_alive: config.default_keep_alive,
            max_keep_alive: config.max_keep_alive,
            ..Default::default()
        }
    }

    pub fn default_keep_alive_secs(&self) -> u16 {
        self.default_keep_alive.as_secs() as u16
    }

    pub fn max_keep_alive_secs(&self) -> u16 {
        self.max_keep_alive.as_secs() as u16
    }

    pub fn session_expiry(&self) -> Duration {
        self.session_expiry
    }

    pub fn set_session_expiry(&mut self, expiry: Duration) {
        self.session_expiry = expiry;
    }

    pub fn active_sessions(&self) -> u32 {
        self.active_sessions
    }

    pub fn inactive_sessions(&self) -> u32 {
        self.inactive_sessions
    }

    /// Inserts a new session into the pool. If the session is connected, the
    /// active session count is incremented and the session is added to the active
    /// session pool. If the session is not connected, the inactive session count
    /// is incremented and the session is added to the inactive session pool.
    pub async fn add(&mut self, session: Arc<RwLock<Session>>) {
        let active;
        let session_id;
        {
            let session = session.read().await;
            active = session.connected();
            session_id = session.id().to_string();
        }
        if active {
            self.active_sessions += 1;
            self.active.insert(session_id, session);
        } else {
            self.inactive_sessions += 1;
            self.inactive.insert(session_id, session);
        }
    }

    /// Removes a session from the pool. The active and inactive counts are decremented
    /// as required and the session is removed from the appropriate pool.
    ///
    pub fn remove(&mut self, session_id: &str) -> Option<Arc<RwLock<Session>>> {
        if let Some(session) = self.active.remove(session_id) {
            self.active_sessions -= 1;
            Some(session)
        } else if let Some(session) = self.inactive.remove(session_id) {
            self.inactive_sessions -= 1;
            Some(session)
        } else {
            None
        }
    }

    /// Retreives an active session from the pool if the session is found in the active
    /// session pool. The session is returned if found, otherwise None is returned.
    /// The session is not removed from the pool.
    ///
    pub fn get_active(&self, session_id: &str) -> Option<Arc<RwLock<Session>>> {
        self.active
            .get(session_id)
            .map(|session| Arc::clone(session))
    }

    /// Removes an active session from the pool. The session is removed from the active
    /// pool and the active session count is decremented. The session is returned if found.
    /// If the session is not found, the function returns None.
    ///
    pub fn remove_active(&mut self, session_id: &str) -> Option<Arc<RwLock<Session>>> {
        if let Some(session) = self.active.remove(session_id) {
            self.active_sessions -= 1;
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
            self.active_sessions += 1;
            self.active
                .insert(session.id().to_string(), Arc::clone(&session_guard));
            self.inactive_sessions -= 1;
            drop(session);
            Some(Arc::clone(&session_guard))
        } else {
            None
        }
    }

    /// Deactivates a session if the active session pool contains the session.
    /// The session is removed from the active pool and added to the inactive pool.
    /// The active session count is decremented and the inactive session count is
    /// incremented.  If the session is not found in the active pool, the function
    /// returns None.
    ///
    /// The session `connected` flag is set to `false` if the session is found.
    pub async fn deactivate(&mut self, session_id: &str) -> Option<Arc<RwLock<Session>>> {
        if let Some(session_guard) = self.active.remove(session_id) {
            let session_id;
            {
                let mut session = session_guard.write().await;
                session.set_connected(false);
                session_id = session.id().to_string();
            }
            self.inactive_sessions += 1;
            self.inactive.insert(session_id, Arc::clone(&session_guard));
            self.active_sessions -= 1;
            Some(session_guard)
        } else {
            None
        }
    }
}

pub(crate) enum SessionControl {
    Disconnect(Reason),
}

#[derive(Debug)]
pub struct Session {
    id: String,
    last_active: Instant,
    connected: bool,
    orphaned: bool,
    keep_alive: Duration,
    pub session_expiry: Option<Duration>,
    will_message: Option<WillMessage>,
    subscriptions: Vec<Subscription>,
    control: (Sender<SessionControl>, Option<Receiver<SessionControl>>),
}

impl Session {
    /// Creates a new session with the last active time set to Instant::now()
    pub fn new(id: String, will_message: Option<WillMessage>, keep_alive: Duration) -> Self {
        let (sender, receiver) = mpsc::channel(10);
        Session {
            id,
            last_active: Instant::now(),
            connected: false,
            orphaned: false,
            keep_alive,
            session_expiry: None,
            will_message,
            subscriptions: Vec::new(),
            control: (sender, Some(receiver)),
        }
    }

    pub fn reset_control(&mut self) {
        let (sender, receiver) = mpsc::channel(10);
        self.control = (sender, Some(receiver));
    }

    pub fn control_sender(&self) -> Sender<SessionControl> {
        self.control.0.clone()
    }

    pub fn take_control_receiver(&mut self) -> Option<Receiver<SessionControl>> {
        self.control.1.take()
    }

    pub async fn disconnect(&self, reason: Reason) {
        // TODO - handle errors and return
        self.control
            .0
            .send(SessionControl::Disconnect(reason))
            .await
            .expect("Failed to send disconnect message");
    }

    /// Clears the will message from the session. This is typically done on a successful
    /// disconnect `0x00` `Success`. See MQTT 5.0 specification,
    /// (3.14.4 DICONNECT Actions)[https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901216]
    pub fn clear_will(&mut self) {
        self.will_message = None;
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

    pub fn expired(&self) -> bool {
        if let Some(session_expiry) = self.session_expiry {
            self.last_active.elapsed() > session_expiry
        } else {
            true
        }
    }

    pub fn session_expiry(&self) -> Option<Duration> {
        self.session_expiry
    }

    pub fn set_session_expiry(&mut self, session_expiry: Duration) {
        self.session_expiry = Some(session_expiry);
    }
}

#[cfg(test)]
pub mod test {

    use super::*;
    use std::time::Duration;

    #[test]
    fn test_session_new() {
        let session = Session::new("test".to_string(), None, Duration::from_secs(10));
        assert_eq!(session.id(), "test");
        assert_eq!(session.connected(), false);
        assert_eq!(session.orphaned(), false);
        assert_eq!(session.keep_alive(), Duration::from_secs(10));
        assert_eq!(session.subscriptions.len(), 0);
        assert_eq!(session.will_message(), None);
        assert_eq!(session.session_expiry, None);
    }

    #[tokio::test]
    async fn test_add_active() {
        let mut pool = SessionPool::new_with_expiry(Duration::from_secs(30));
        let mut session = Session::new("test".to_string(), None, Duration::from_secs(10));
        session.set_connected(true);
        let session = Arc::new(RwLock::new(session));
        pool.add(session).await;
        assert_eq!(pool.active_sessions(), 1);
        assert_eq!(pool.inactive_sessions(), 0);
    }

    #[tokio::test]
    async fn test_add_inactive() {
        let mut pool = SessionPool::new_with_expiry(Duration::from_secs(30));
        let session = Arc::new(RwLock::new(Session::new(
            "test".to_string(),
            None,
            Duration::from_secs(10),
        )));
        pool.add(session).await;
        assert_eq!(pool.active_sessions(), 0);
        assert_eq!(pool.inactive_sessions(), 1);
    }

    #[tokio::test]
    async fn test_session_activate() {
        let mut pool = SessionPool::new_with_expiry(Duration::from_secs(30));
        let session = Arc::new(RwLock::new(Session::new(
            "test_1".to_string(),
            None,
            Duration::from_secs(10),
        )));
        pool.add(session).await;
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

    #[tokio::test]
    async fn test_session_deactivate() {
        let mut pool = SessionPool::new_with_expiry(Duration::from_secs(30));
        let mut session = Session::new("test".to_string(), None, Duration::from_secs(10));
        session.set_connected(true);
        let session = Arc::new(RwLock::new(session));
        pool.add(session).await;
        pool.deactivate("test").await;
        assert_eq!(pool.active_sessions(), 0);
        assert_eq!(pool.inactive_sessions(), 1);
        // get the session and validate connected flag
        let session: &Arc<RwLock<Session>> = pool.inactive.get("test").unwrap();
        let session = session.read().await;
        assert_eq!(session.connected(), false);
    }
}
