//! The write timer (coalesced `Write` runs), suspend and the exit flush.

use std::sync::Mutex;

use kit_sync_types::SyncTrigger;
use tokio::task::AbortHandle;
use tracing::warn;

use crate::budget::{BudgetExceeded, with_budget};
use crate::host::Host;
use crate::lock;
use crate::run::{active, request, run};

/// The armed write timer, if any.
#[derive(Debug, Default)]
pub(crate) struct WriteTimer(Mutex<Option<AbortHandle>>);

impl WriteTimer {
    fn replace(&self, timer: AbortHandle) {
        if let Some(previous) = lock(&self.0).replace(timer) {
            previous.abort();
        }
    }

    fn cancel(&self) {
        if let Some(timer) = lock(&self.0).take() {
            timer.abort();
        }
    }

    /// Armed and not fired yet.
    pub(crate) fn armed(&self) -> bool {
        lock(&self.0)
            .as_ref()
            .is_some_and(|timer| !timer.is_finished())
    }
}

/// A local save happened: (re)arm the `Write` timer and reset the backoff.
/// No-op when automatic runs are off.
pub fn schedule_after_write<H: Host>(host: &H) {
    let Some(auto) = active(host) else { return };
    auto.backoff.reset();
    let timer_host = host.clone();
    let delay = auto.policy.write_delay;
    let timer = auto.rt.spawn(async move {
        tokio::time::sleep(delay).await;
        request(&timer_host, SyncTrigger::Write);
    });
    auto.write_timer.replace(timer.abort_handle());
}

/// The app is paused: drop the write timer and `Flush` if anything is
/// pending.
///
/// The pending check runs inside the kit and has no hook, so an app cannot
/// act on its answer. octo-budget-rs enqueued its Android background job only
/// when something was pending (saks/octo-budget-rs#149). With this function
/// it has two options: call [`Host::pending`] itself before enqueueing,
/// which counts the pending rows a second time (the kit counts them once
/// here), or enqueue unconditionally (the job then finds nothing pending).
pub fn on_suspended<H: Host>(host: &H) {
    let Some(auto) = active(host) else { return };
    auto.write_timer.cancel();
    let host = host.clone();
    auto.rt.spawn(async move {
        // A failed count flushes anyway.
        if !matches!(host.pending().await, Ok(0)) {
            request(&host, SyncTrigger::Flush);
        }
    });
}

/// The process is about to exit: drop the write timer and push what is
/// pending, within [`crate::SyncPolicy::exit_budget`]. Does nothing when
/// automatic runs are off. The app blocks on it (for example in its exit
/// event) and exits either way. An engine follow-up after this run
/// ([`Host::wants_follow_up`]) is skipped: no flight starts during shutdown,
/// and what is left stays queued for the next launch.
pub async fn flush_before_exit<H: Host>(host: &H) -> Result<(), BudgetExceeded> {
    let Some(auto) = active(host) else {
        return Ok(());
    };
    auto.write_timer.cancel();
    let flush = async {
        if !matches!(host.pending().await, Ok(0)) {
            // `run` already logs and emits the outcome.
            let _outcome = run(host, SyncTrigger::Exit).await;
        }
    };
    with_budget(auto.policy.exit_budget, flush)
        .await
        .inspect_err(|_| warn!("sync: exit flush ran out of time; exiting anyway"))
}
