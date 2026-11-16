//! One background run, as `SyncWorker.runSync` asks for it.

use std::path::Path;

use tracing::warn;

use crate::config::BackgroundSync;
use crate::result::{PANIC_CODE, WorkerResult};

/// Why the worker could not even start the app's run.
#[derive(Debug)]
pub(crate) enum NoDataDir {
    /// The `String` could not be read from Java.
    Unreadable(String),
    /// Reading it panicked.
    Panicked,
}

/// What one background run tells `SyncWorker`.
///
/// [`RunOutcome::Panicked`] is not a [`WorkerResult`]. The app's `run` cannot
/// return it. Its JNI code is [`PANIC_CODE`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RunOutcome {
    /// The app's `run` returned, or the data dir string could not be read.
    Finished(WorkerResult),
    /// A panic was caught, so it does not cross into Java.
    Panicked,
}

impl RunOutcome {
    /// The `Int` `SyncWorker.runSync` returns.
    pub(crate) fn code(self) -> i32 {
        match self {
            Self::Finished(result) => result.code(),
            Self::Panicked => PANIC_CODE,
        }
    }
}

/// Get the app's job from `registered`, call its `init_logs` first, then run
/// it in `data_dir`. An unreadable data dir fails the job. A panic in the
/// app's registration, its `init_logs`, its run, or while reading the data
/// dir, becomes [`RunOutcome::Panicked`]: the panic must not cross into Java.
/// `SyncWorker` retries that once. Release builds that abort on panic die
/// instead; `SyncWorker` stops the job after repeated deaths.
pub(crate) fn run(
    registered: fn() -> BackgroundSync,
    data_dir: Result<String, NoDataDir>,
) -> RunOutcome {
    std::panic::catch_unwind(|| run_registered(registered(), data_dir)).unwrap_or_else(|_| {
        warn!("sync: background job panicked");
        RunOutcome::Panicked
    })
}

fn run_registered(job: BackgroundSync, data_dir: Result<String, NoDataDir>) -> RunOutcome {
    if let Some(init_logs) = job.init_logs {
        init_logs();
    }
    let data_dir = match data_dir {
        Ok(dir) => dir,
        Err(NoDataDir::Unreadable(err)) => {
            warn!(error = %err, "sync: background job got no data dir; failing");
            return RunOutcome::Finished(WorkerResult::Failure);
        }
        Err(NoDataDir::Panicked) => {
            warn!("sync: background job panicked reading the data dir");
            return RunOutcome::Panicked;
        }
    };
    RunOutcome::Finished((job.run)(Path::new(&data_dir)))
}

#[cfg(test)]
mod tests;
