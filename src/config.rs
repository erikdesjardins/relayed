use std::ops::RangeInclusive;
use std::time::Duration;

pub const BUFFER_SIZE: usize = 4096;

pub const QUEUE_TIMEOUT: Duration = Duration::from_secs(60);
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
pub const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(10);
pub const KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(10);

pub const BACKOFF_SECS: RangeInclusive<usize> = 1..=64;
