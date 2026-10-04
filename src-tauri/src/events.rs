use std::time::Duration;

use nimata_core::time::UnixMillis;
use tauri::{AppHandle, Emitter};

/// Name of the event that tells the UI to refresh relative timestamps.
pub const CLOCK_TICK: &str = "nimata://clock-tick";

/// Emits the current time every 30 seconds so relative timestamps such as
/// "5 min ago" stay current without each component running its own timer.
pub fn start_clock(app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(30));
            if app.emit(CLOCK_TICK, UnixMillis::now()).is_err() {
                break;
            }
        }
    });
}
