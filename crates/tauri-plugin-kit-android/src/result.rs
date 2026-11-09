//! What the worker tells WorkManager.

/// `SyncWorker.doWork`'s answer. The numbers are the JNI contract with
/// `SyncWorker.kt`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum WorkerResult {
    /// Done: synced, nothing pending, or Sync is off.
    Success = 0,
    /// Try again with WorkManager's backoff: offline, busy, or a panic.
    Retry = 1,
    /// Retrying can't help until the user acts (token rejected…).
    Failure = 2,
}

impl WorkerResult {
    /// The `Int` `SyncWorker.kt` maps to `Result.success/retry/failure`.
    pub fn code(self) -> i32 {
        self as i32
    }
}

#[cfg(test)]
mod tests;
