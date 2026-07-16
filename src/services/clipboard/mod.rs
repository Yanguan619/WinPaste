/// Clipboard monitoring service.
/// Monitors the Windows clipboard for changes and captures new entries.
/// Full implementation will be ported from WinPaste's clipboard pipeline.
use std::sync::atomic::{AtomicBool, Ordering};

use crate::info;

static RUNNING: AtomicBool = AtomicBool::new(false);

/// Start the clipboard monitoring service in a background thread.
/// Spawns a tokio task that polls the clipboard sequence number and
/// processes new clipboard content through the pipeline.
pub fn start() {
    if RUNNING.swap(true, Ordering::Relaxed) {
        return; // Already running
    }
    info!("Clipboard monitor starting...");
    // Stub: spawn clipboard monitoring loop
}

/// Stop the clipboard monitoring service.
pub fn stop() {
    RUNNING.store(false, Ordering::Relaxed);
    info!("Clipboard monitor stopped");
}

/// Returns whether the clipboard monitor is currently running.
pub fn is_running() -> bool {
    RUNNING.load(Ordering::Relaxed)
}
