//! What the app hands the worker.

use std::borrow::Cow;
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use kit_core::LogTag;
use kit_core::const_str::char_at;

use crate::result::WorkerResult;

/// The app's background job, registered with [`crate::background_sync!`].
#[derive(Clone, Debug)]
pub struct BackgroundSync {
    /// Called first by the worker, before it reads the data dir or calls
    /// [`BackgroundSync::run`], so the worker's own early warnings are
    /// logged in a cold process (where no app code has run yet). Install the
    /// app's logger here; it must tolerate being called more than once in a
    /// process (one call per job). `None`: the app sets up logging in `run`,
    /// and the worker's warnings before it may be lost.
    pub init_logs: Option<fn()>,
    /// One background Sync in the app's data dir (Tauri's `app_data_dir`).
    /// Runs on WorkManager's thread, not a tokio one, so it may `block_on`.
    pub run: fn(&Path) -> WorkerResult,
    /// The app's native library, which `SyncWorker` loads in a cold process.
    pub lib_name: LibName,
    /// Logcat tag for the worker's own Kotlin warnings.
    pub log_tag: LogTag,
    /// When WorkManager runs the job.
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
///
/// Registration code uses [`crate::lib_name!`], which checks the literal at
/// compile time, so building a [`BackgroundSync`] cannot fail at run time.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LibName(Cow<'static, str>);

/// A `&'static str` already checked as a [`LibName`], from
/// [`LibName::literal`]. `Copy` with no drop, so `const` code can match on
/// `Result<LibNameLiteral, _>` (it cannot on `Result<LibName, _>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LibNameLiteral(&'static str);

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

/// The `LibName` rules, `const` so a literal can be checked at compile time.
/// The allowed characters are ASCII, so a byte is a character until the
/// first non-ASCII byte, which is reported as the `char` it starts.
const fn check(raw: &str) -> Result<(), LibNameError> {
    let bytes = raw.as_bytes();
    if bytes.is_empty() {
        return Err(LibNameError::Empty);
    }
    if ends_with_so(bytes) {
        return Err(LibNameError::FileName);
    }
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        let ok = byte == b'_' || byte.is_ascii_alphabetic() || (at > 0 && byte.is_ascii_digit());
        if !ok {
            return Err(LibNameError::InvalidChar {
                at,
                ch: first_char(raw, at),
            });
        }
        at += 1;
    }
    Ok(())
}

const fn ends_with_so(bytes: &[u8]) -> bool {
    let len = bytes.len();
    len >= 3 && bytes[len - 3] == b'.' && bytes[len - 2] == b's' && bytes[len - 1] == b'o'
}

/// The `char` at byte `at`, which follows only ASCII bytes.
const fn first_char(raw: &str, at: usize) -> char {
    char_at(raw.as_bytes(), at).0
}

impl FromStr for LibName {
    type Err = LibNameError;

    fn from_str(raw: &str) -> Result<Self, LibNameError> {
        check(raw)?;
        Ok(Self(Cow::Owned(raw.to_owned())))
    }
}

impl LibName {
    /// Checks a literal with the same rules as `parse`. `const`, so
    /// [`crate::lib_name!`] runs it at compile time.
    pub const fn literal(raw: &'static str) -> Result<LibNameLiteral, LibNameError> {
        match check(raw) {
            Ok(()) => Ok(LibNameLiteral(raw)),
            Err(err) => Err(err),
        }
    }

    /// The checked literal, borrowed rather than copied.
    pub const fn from_literal(literal: LibNameLiteral) -> Self {
        Self(Cow::Borrowed(literal.0))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests;
