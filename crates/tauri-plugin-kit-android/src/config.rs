//! What the app hands the worker.

use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use kit_core::LogTag;

use crate::result::WorkerResult;

/// The app's background job, registered with [`crate::background_sync!`].
#[derive(Clone, Debug)]
pub struct BackgroundSync {
    /// One background Sync in the app's data dir (Tauri's `app_data_dir`).
    /// Runs on WorkManager's thread, not a tokio one, so it may `block_on`.
    /// In a cold process it is the first app code that runs: set up logging
    /// there.
    pub run: fn(&Path) -> WorkerResult,
    /// The app's native library, which `SyncWorker` loads in a cold process.
    pub lib_name: LibName,
    /// Logcat tag for the worker's own Kotlin warnings.
    pub log_tag: LogTag,
    pub schedule: Schedule,
}

/// When WorkManager runs the job.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schedule {
    /// Wait before the first attempt, so an in-app Sync can go first (the
    /// job then finds nothing pending and makes no network call).
    pub initial_delay: Duration,
    /// WorkManager's exponential backoff start (it raises anything under
    /// 10 s to 10 s).
    pub backoff: Duration,
}

/// The app's native library name as `System.loadLibrary` takes it:
/// `sickness_tracker_lib` for `libsickness_tracker_lib.so`. ASCII letters,
/// digits and `_`, not starting with a digit.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LibName(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibNameError {
    Empty,
    /// Ends in `.so`: give the bare name.
    FileName,
    InvalidChar {
        at: usize,
        ch: char,
    },
}

impl fmt::Display for LibNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("library name is empty"),
            Self::FileName => f.write_str("library name is the bare name, without `.so`"),
            Self::InvalidChar { at, ch } => {
                write!(f, "library name has {ch:?} at position {at}")
            }
        }
    }
}

impl std::error::Error for LibNameError {}

impl FromStr for LibName {
    type Err = LibNameError;

    fn from_str(raw: &str) -> Result<Self, LibNameError> {
        if raw.is_empty() {
            return Err(LibNameError::Empty);
        }
        if raw.ends_with(".so") {
            return Err(LibNameError::FileName);
        }
        for (at, ch) in raw.chars().enumerate() {
            let ok = ch == '_' || ch.is_ascii_alphabetic() || (at > 0 && ch.is_ascii_digit());
            if !ok {
                return Err(LibNameError::InvalidChar { at, ch });
            }
        }
        Ok(Self(raw.to_string()))
    }
}

impl LibName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests;
