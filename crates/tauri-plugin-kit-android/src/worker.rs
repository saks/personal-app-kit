//! One background run, as `SyncWorker.runSync` asks for it.

use std::path::Path;

use tracing::warn;

use crate::config::BackgroundSync;
use crate::result::WorkerResult;

/// Why the worker could not even start the app's run.
#[derive(Debug)]
pub(crate) enum NoDataDir {
    /// The `String` could not be read from Java.
    Unreadable(String),
    /// Reading it panicked.
    Panicked,
}

/// Run the app's job in `data_dir`. A missing data dir or a panic in the
/// app's run becomes a retry: a panic must not cross into Java. (Release
/// builds that abort on panic die instead, and WorkManager reschedules.)
pub(crate) fn run(job: &BackgroundSync, data_dir: Result<String, NoDataDir>) -> WorkerResult {
    let data_dir = match data_dir {
        Ok(dir) => dir,
        Err(NoDataDir::Unreadable(err)) => {
            warn!(error = %err, "sync: background job got no data dir; WorkManager retries");
            return WorkerResult::Retry;
        }
        Err(NoDataDir::Panicked) => {
            warn!("sync: background job panicked reading the data dir; WorkManager retries");
            return WorkerResult::Retry;
        }
    };
    let run = job.run;
    std::panic::catch_unwind(|| run(Path::new(&data_dir))).unwrap_or_else(|_| {
        warn!("sync: background job panicked; WorkManager retries");
        WorkerResult::Retry
    })
}

#[cfg(test)]
mod tests;
