use std::ops::RangeInclusive;
use std::time::Duration;

pub const MIN_BUFFER_SIZE: usize = 4 * 1024;
pub const MAX_BUFFER_SIZE: usize = 2 * 1024 * 1024;

pub const QUEUE_TIMEOUT: Duration = Duration::from_secs(60);
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
pub const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(10);

pub const SERVER_ACCEPT_BACKOFF_SECS: RangeInclusive<u8> = 1..=64;
pub const CLIENT_BACKOFF_SECS: RangeInclusive<u8> = 1..=64;
