//! What the worker tells WorkManager.

/// `SyncWorker.doWork`'s answer. The numbers are the JNI contract with
/// `SyncWorker.kt`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum WorkerResult {
    /// Done: synced, nothing pending, or Sync is off.
    Success = 0,
    /// Try again with WorkManager's backoff: offline or busy.
    /// A panic is not a retry. `SyncWorker` handles that on its own.
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

/// `AttemptBudget.kt`'s `PANIC`. Not a [`WorkerResult`] variant: the app's
/// `run` cannot return it. A caught panic uses this code. `SyncWorker`
/// retries it once, then fails the job.
///
/// Host builds omit it: only the Android worker and the tests name it, and
/// the workspace forbids an unused constant.
#[cfg(any(target_os = "android", test))]
pub(crate) const PANIC_CODE: i32 = 3;

#[cfg(test)]
mod tests;
