//! What started a Sync.

use serde::{Deserialize, Serialize};

/// What started a Sync: the union of both apps' triggers. An app that never
/// fires a variant still matches it. Serialized `snake_case` (every name is
/// one word, so it reads the same as `camelCase`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncTrigger {
    /// App start or return to the foreground.
    Auto,
    /// The user's *Sync now*.
    Manual,
    /// The host, shortly after a local save (coalesced).
    Write,
    /// The host, when the app is paused.
    Flush,
    /// The host, retrying a failed automatic Sync after a backoff delay.
    Retry,
    /// The webview reports the network is back (`online`).
    Online,
    /// A background job (Android WorkManager), possibly without a UI.
    Background,
    /// The host, pushing what is queued before the process exits.
    Exit,
    /// The host, after saved settings left Sync configured.
    Settings,
}

impl SyncTrigger {
    /// Not a user's *Sync now*: its failures count towards the backoff.
    pub fn automatic(self) -> bool {
        match self {
            Self::Manual => false,
            Self::Auto
            | Self::Write
            | Self::Flush
            | Self::Retry
            | Self::Online
            | Self::Background
            | Self::Exit
            | Self::Settings => true,
        }
    }

    /// Started by the host, so no UI call gets its result.
    pub fn from_host(self) -> bool {
        match self {
            Self::Write
            | Self::Flush
            | Self::Retry
            | Self::Background
            | Self::Exit
            | Self::Settings => true,
            Self::Auto | Self::Manual | Self::Online => false,
        }
    }
}

#[cfg(test)]
mod tests;
