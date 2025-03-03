use std::{net::SocketAddr, time::Duration};

const DEFAULT_MAX_TOPIC_NAME_LEN: usize = 4096;
const DEFAULT_MAX_TOPIC_MEM: usize = 1024 * 1000;
pub(crate) const DEFAULT_KEEP_ALIVE_SECS: u16 = 30;
pub(crate) const MAX_KEEP_ALIVE_AS_SECS: u16 = 120;
pub(crate) const BROKER_KEEP_ALIVE_FACTOR: f32 = 1.5;

pub const DEFAULT_PORT: u16 = 1883;
pub const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1";
pub const DEFAULT_KEEP_ALIVE: Duration =
    Duration::from_secs((DEFAULT_KEEP_ALIVE_SECS as f32 * BROKER_KEEP_ALIVE_FACTOR) as u64);
pub const MAX_KEEP_ALIVE: Duration =
    Duration::from_secs((MAX_KEEP_ALIVE_AS_SECS as f32 * BROKER_KEEP_ALIVE_FACTOR) as u64);
// 10 minute session expiration
pub const DEFAULT_SESSION_EXPIRY: Duration = Duration::from_secs(60 * 10);

#[derive(Debug, Clone)]
pub struct Config {
    pub listen_addr: SocketAddr,
    pub default_keep_alive: Duration,
    pub max_keep_alive: Duration,
    pub session_expiry: Duration,
    pub max_topic_name_len: usize,
    pub max_topic_mem: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: SocketAddr::from(([127, 0, 0, 1], 1883)),
            default_keep_alive: DEFAULT_KEEP_ALIVE,
            max_keep_alive: MAX_KEEP_ALIVE,
            session_expiry: DEFAULT_SESSION_EXPIRY,
            max_topic_name_len: DEFAULT_MAX_TOPIC_NAME_LEN,
            max_topic_mem: DEFAULT_MAX_TOPIC_MEM,
        }
    }
}

impl Config {
    pub fn new(listen_addr: SocketAddr) -> Self {
        Self {
            listen_addr,
            ..Default::default()
        }
    }
}
