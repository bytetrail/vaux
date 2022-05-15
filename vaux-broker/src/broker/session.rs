use std::time::{Duration, Instant};

const DEFAULT_SESSION_EXPIRY_INTERVAL: u64 = 60 * 30; // 30 minute session expiry
const DEFAULT_SESSION_SCAN_PERIOD: u64 = 60; // scan for expired sessions 1 minute

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    last_active: Instant,
    max_keep_alive: Duration,
    pub expiry: Duration,
}

impl Session {
    /// Creates a new session with the last active time set to Instant::now()
    pub fn new(id: String, max_keep_alive: Duration) -> Self {
        Session {
            id,
            last_active: Instant::now(),
            max_keep_alive,
            expiry: Duration::new(0, 0),
        }
    }

    pub fn new_with_expiry(id: String, max_keep_alive_secs: u32, expiry_secs: u32) -> Self {
        Session {
            id,
            last_active: Instant::now(),
            max_keep_alive: Duration::from_secs(max_keep_alive_secs as u64),
            expiry: Duration::from_secs(expiry_secs as u64),
        }
    }

    /// Sets the last session activity to the time that the method is invoked.
    pub fn set_last_active(&mut self) {
        self.last_active = Instant::now();
    }

    /// Gets the maximum keep alive in seconds.
    pub fn max_keep_alive_secs(&self) -> u64 {
        self.max_keep_alive.as_secs()
    }
    /// Sets the maximum session keep alive to the time passed in seconds.
    pub fn set_max_keep_alive_secs(&mut self, secs: u64) {
        self.max_keep_alive = Duration::from_secs(secs);
    }

    /// Returns true if the session keep alive time exceeds the session maximum
    /// keep alive time based on the std::time::Instant that the method is invoked.
    pub fn max_keep_alive_exceeded(&self) -> bool {
        self.last_active.elapsed() > self.max_keep_alive
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
        assert_eq!(Duration::from_secs(600), session.expiry);
        assert_eq!(Duration::from_secs(60), session.max_keep_alive);
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
