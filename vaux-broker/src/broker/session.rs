use std::time::{Duration, Instant};

use super::DEFAULT_KEEP_ALIVE;

const DEFAULT_SESSION_EXPIRY_INTERVAL: u64 = 60 * 30; // 30 minute session expiry
const DEFAULT_SESSION_SCAN_PERIOD: u64 = 60; // scan for expired sessions 1 minute

#[derive(Debug, Clone)]
pub struct Session {
    id: String,
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
            id,
            last_active: Instant::now(),
            connected: true,
            orphaned: false,
            keep_alive,
            session_expiry: Duration::new(0, 0),
        }
    }

    pub fn new_with_expiry(id: String, keep_alive_secs: u32, expiry_secs: u32) -> Self {
        Session {
            id,
            last_active: Instant::now(),
            connected: true,
            orphaned: false,
            keep_alive: Duration::from_secs(keep_alive_secs as u64),
            session_expiry: Duration::from_secs(expiry_secs as u64),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
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

    pub(crate) fn set_orphaned(&mut self)  {
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

pub struct SessionManager {
    pub max_active: u32,
    pub max_sessions: u32,
    scan_period: Duration,
    default_expiry: Duration,
}

impl SessionManager {
    pub fn new(max_sessions: u32, max_active: u32) -> Self {
        SessionManager {
            max_active,
            max_sessions,
            scan_period: Duration::from_secs(DEFAULT_SESSION_SCAN_PERIOD),
            default_expiry: Duration::from_secs(DEFAULT_SESSION_EXPIRY_INTERVAL),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_new_with_expiry() {
        let session = Session::new_with_expiry("test_01".to_string(), 60, 600);
        assert_eq!(Duration::from_secs(600), session.session_expiry);
        assert_eq!(Duration::from_secs(60), session.keep_alive);
    }

    #[test]
    fn test_session_manager_new() {
        let mgr = SessionManager::new(100, 50);
        assert_eq!(
            Duration::from_secs(DEFAULT_SESSION_SCAN_PERIOD),
            mgr.scan_period
        );
        assert_eq!(
            Duration::from_secs(DEFAULT_SESSION_EXPIRY_INTERVAL),
            mgr.default_expiry
        );
    }
}
