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
///
/// Both delays are a whole number of milliseconds, so the bridge sends
/// the value unchanged. `backoff` is from [`Self::MIN_BACKOFF`] (10 s) to
/// [`Self::MAX_BACKOFF`] (5 h): WorkManager's own clamp, rejected here
/// instead of logged later. `initial_delay` is at most
/// [`Self::MAX_INITIAL_DELAY`], which `setInitialDelay` accepts. Build one
/// with [`Self::new`] or [`crate::schedule!`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schedule {
    initial_delay: Duration,
    backoff: Duration,
}

/// Why [`Schedule::new`] rejected a delay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleError {
    /// `initial_delay` or `backoff` is not a whole number of milliseconds.
    SubMillisecond,
    /// `backoff` is under 10 s or over 5 h.
    Backoff,
    /// `initial_delay` is longer than WorkManager will accept.
    InitialDelay,
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SubMillisecond => f.write_str("delay has a fractional millisecond"),
            Self::Backoff => f.write_str("backoff must be from 10 s to 5 h"),
            Self::InitialDelay => f.write_str("initial delay is too long for WorkManager"),
        }
    }
}

impl std::error::Error for ScheduleError {}

impl Schedule {
    /// WorkManager's `MIN_BACKOFF_MILLIS` (10 s).
    pub const MIN_BACKOFF: Duration = Duration::from_millis(10_000);
    /// WorkManager's `MAX_BACKOFF_MILLIS` (5 h).
    pub const MAX_BACKOFF: Duration = Duration::from_millis(5 * 60 * 60 * 1000);
    /// Largest `initial_delay` [`Schedule::new`] accepts.
    ///
    /// `setInitialDelay` throws when
    /// `Long.MAX_VALUE - System.currentTimeMillis() <= delay`.
    /// `currentTimeMillis` stays below `i64::MAX / 2` for longer than this
    /// crate will, so a delay of at most that many milliseconds cannot throw.
    pub const MAX_INITIAL_DELAY: Duration = Duration::from_millis((i64::MAX / 2) as u64);

    /// Checks `initial_delay` and `backoff` against WorkManager's limits.
    ///
    /// A fractional millisecond is [`ScheduleError::SubMillisecond`].
    /// `backoff` outside [`Self::MIN_BACKOFF`]..=[`Self::MAX_BACKOFF`] is
    /// [`ScheduleError::Backoff`]. An `initial_delay` past
    /// [`Self::MAX_INITIAL_DELAY`] is [`ScheduleError::InitialDelay`].
    /// When more than one rule fails, the first of those is the error.
    pub const fn new(initial_delay: Duration, backoff: Duration) -> Result<Self, ScheduleError> {
        // `?` is not const-stable, so this matches.
        let initial_ms = match whole_millis(initial_delay) {
            Ok(ms) => ms,
            Err(err) => return Err(err),
        };
        let backoff_ms = match whole_millis(backoff) {
            Ok(ms) => ms,
            Err(err) => return Err(err),
        };
        if backoff_ms < Self::MIN_BACKOFF.as_millis() || backoff_ms > Self::MAX_BACKOFF.as_millis()
        {
            return Err(ScheduleError::Backoff);
        }
        if initial_ms > Self::MAX_INITIAL_DELAY.as_millis() {
            return Err(ScheduleError::InitialDelay);
        }
        Ok(Self {
            initial_delay,
            backoff,
        })
    }

    /// Wait before the first attempt, so an in-app Sync can go first (the
    /// job then finds nothing pending and makes no network call).
    pub const fn initial_delay(self) -> Duration {
        self.initial_delay
    }

    /// WorkManager's exponential backoff start: a whole number of
    /// milliseconds from 10 s to 5 h.
    pub const fn backoff(self) -> Duration {
        self.backoff
    }

    /// Milliseconds for `SyncWorker.enqueue`. `new` only stores a whole
    /// millisecond count up to `i64::MAX / 2`, so the cast does not saturate.
    // Host builds without `test` have no caller: the glue is Android-only.
    #[cfg(any(target_os = "android", test))]
    pub(crate) const fn millis(self) -> [i64; 2] {
        [
            self.initial_delay.as_millis() as i64,
            self.backoff.as_millis() as i64,
        ]
    }
}

/// The millisecond count, or [`ScheduleError::SubMillisecond`] when `wait`
/// has a fractional millisecond. The caller checks the range.
const fn whole_millis(wait: Duration) -> Result<u128, ScheduleError> {
    let nanos = wait.as_nanos();
    if !nanos.is_multiple_of(1_000_000) {
        return Err(ScheduleError::SubMillisecond);
    }
    Ok(nanos / 1_000_000)
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
