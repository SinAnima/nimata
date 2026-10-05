//! Timestamps are UTC instants in Unix milliseconds plus the author's UTC
//! offset at the moment of writing. Ordering only ever uses the instant.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnixMillis(pub i64);

impl UnixMillis {
    pub fn now() -> Self {
        Self(chrono::Utc::now().timestamp_millis())
    }
}

/// An instant together with the local UTC offset in effect when it was
/// recorded, read from the device clock at the same moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamp {
    pub at: UnixMillis,
    pub offset_minutes: i32,
}

impl Timestamp {
    pub fn now() -> Self {
        let local = chrono::Local::now();
        Self {
            at: UnixMillis(local.timestamp_millis()),
            offset_minutes: local.offset().local_minus_utc() / 60,
        }
    }
}
