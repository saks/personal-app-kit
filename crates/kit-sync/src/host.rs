//! What the scheduler needs from the app, and what it reports back.

use std::fmt::Display;
use std::future::Future;

use kit_sync_types::{FailureClass, SyncTrigger};

use crate::AutoSync;

/// The app's side of the scheduler: its engine, its events and its retry
/// rule. Production implements it on the app handle; tests on a fake.
pub trait Host: Clone + Send + Sync + 'static {
    /// What one successful Sync reports.
    type Report: Send;
    /// Why one Sync failed.
    type Error: Display + Send;

    /// The scheduler, or `None` while the app state is not ready.
    fn autosync(&self) -> Option<&AutoSync>;

    /// How many local changes are waiting to be pushed. An error counts as
    /// "maybe some": the scheduler syncs anyway.
    fn pending(&self) -> impl Future<Output = Result<u64, Self::Error>> + Send;

    /// One Sync. The engine takes and releases its own locks; the kit holds
    /// none across this call.
    fn sync(&self) -> impl Future<Output = Result<Self::Report, Self::Error>> + Send;

    /// Report a run starting or finishing (the app maps it to its UI event).
    fn emit(&self, event: &SyncEvent<'_, Self::Report, Self::Error>);

    /// The retry rule: the app classifies its own error.
    fn class(&self, error: &Self::Error) -> FailureClass;

    /// After a successful run: whether the engine left work behind (rows saved
    /// during the push it didn't take) and wants one more run queued. The
    /// default never asks.
    fn wants_follow_up(&self, _report: &Self::Report) -> bool {
        false
    }

    /// The app was paused and the kit counted what is unpushed. Called once
    /// per [`crate::on_suspended`], after the count and before any `Flush` is
    /// requested. `pending` is the count, or `Err` when it failed (the kit
    /// flushes anyway). Use it to enqueue a background job only when there
    /// is work.
    ///
    /// Runs on the scheduler runtime, inside the task `on_suspended` spawned,
    /// not on the caller's stack. Not called when automatic runs are inactive
    /// ([`crate::AutoSync::active`] is false): `on_suspended` returns before
    /// the count. An app that wants a background job then decides that itself.
    fn suspended(&self, pending: Result<u64, &Self::Error>) {
        let _ = pending;
    }
}

/// A run starting or finishing, borrowed from [`crate::run`].
#[derive(Debug)]
pub enum SyncEvent<'a, R, E> {
    /// Sent before [`Host::sync`] (which may still wait on an engine lock).
    Started {
        /// What started the run.
        trigger: SyncTrigger,
    },
    /// Sent after [`Host::sync`] returned.
    Finished {
        /// What started the run.
        trigger: SyncTrigger,
        /// How it went.
        outcome: Outcome<'a, R, E>,
    },
}

/// How one run went.
#[derive(Debug)]
pub enum Outcome<'a, R, E> {
    /// The engine's report.
    Synced(&'a R),
    /// The engine's error, with the class [`Host::class`] gave it.
    Failed {
        /// Whether a retry may clear it.
        class: FailureClass,
        /// The error itself.
        error: &'a E,
    },
}
