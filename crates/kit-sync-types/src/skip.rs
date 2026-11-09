//! Why a Sync request did nothing.

use serde::{Deserialize, Serialize};

/// Why a Sync request did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncSkip {
    /// No connection (or key) on this device.
    NotConfigured,
    /// Another Sync is running; it does a follow-up run instead.
    AlreadyRunning,
    /// An automatic trigger inside the app's debounce window.
    Debounced,
}

#[cfg(test)]
mod tests;
