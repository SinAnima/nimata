//! Timestamps are UTC instants in Unix milliseconds plus the author's UTC
//! offset at the moment of writing. Ordering only ever uses the instant.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnixMillis(pub i64);

impl UnixMillis {
    pub fn now() -> Self {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before 1970");
        Self(elapsed.as_millis() as i64)
    }

    pub fn minus_minutes(self, minutes: i64) -> Self {
        Self(self.0 - minutes * 60_000)
    }
}
