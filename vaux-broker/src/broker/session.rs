use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Session {
    _id: String,
    last_active: Instant,
    connected: bool,
    orphaned: bool,
    keep_alive: Duration,
    pub session_expiry: Duration,
}

impl Session {
    /// Creates a new session with the last active time set to Instant::now()
    pub fn new(id: String, keep_alive: Duration) -> Self {
        Session {
            _id: id,
            last_active: Instant::now(),
            connected: true,
            orphaned: false,
            keep_alive,
            session_expiry: Duration::new(0, 0),
        }
    }

    pub fn connected(&self) -> bool {
        self.connected
    }

    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }

    pub fn orphaned(&self) -> bool {
        self.orphaned
    }

    pub(crate) fn set_orphaned(&mut self) {
        self.orphaned = true;
    }

    /// Sets the last session activity to the time that the method is invoked.
    pub fn set_last_active(&mut self) {
        self.last_active = Instant::now();
    }

    /// Gets the maximum keep alive in seconds.
    pub fn keep_alive(&self) -> u64 {
        self.keep_alive.as_secs()
    }
    /// Sets the maximum session keep alive to the time passed in seconds.
    pub fn set_keep_alive(&mut self, secs: u64) {
        self.keep_alive = Duration::from_secs(secs);
    }
}
