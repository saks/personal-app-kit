//! [`run`], the only entry point of a Sync, and the automatic request loop.

use kit_sync_types::{FailureClass, SyncTrigger};
use tracing::{info, warn};

use crate::AutoSync;
use crate::backoff::arm_retry;
use crate::flight::{Admission, Flight};
use crate::host::{Host, Outcome, SyncEvent};

/// The only way a Sync runs. Emits `Started`, runs one [`Host::sync`], emits
/// `Finished`. A success resets the backoff and clears `NeedsUser`, and may
/// queue one follow-up ([`Host::wants_follow_up`]); a `NeedsUser` failure
/// closes the gate and drops any armed retry. A Manual run calls this
/// directly (it is not admitted into the flight; the app's engine lock keeps
/// it from overlapping) and never arms a retry.
///
/// The follow-up is always automatic: after a Manual run it is queued as
/// `Write` (the rows it covers were saved during the push), so the UI sees
/// no second Manual start and an offline failure arms a retry. After an
/// `Exit` run there is none: the process is shutting down.
pub async fn run<H: Host>(host: &H, trigger: SyncTrigger) -> Result<H::Report, H::Error> {
    info!(?trigger, "sync: start");
    host.emit(&SyncEvent::Started { trigger });
    let result = host.sync().await;
    match &result {
        Ok(report) => {
            if let Some(auto) = host.autosync() {
                auto.shared.clear_needs_user();
                auto.backoff.reset();
            }
            info!(?trigger, "sync: finished");
            host.emit(&SyncEvent::Finished {
                trigger,
                outcome: Outcome::Synced(report),
            });
            if host.wants_follow_up(report) {
                queue_follow_up(host, trigger);
            }
        }
        Err(error) => {
            let class = host.class(error);
            if let (Some(auto), FailureClass::NeedsUser) = (host.autosync(), class) {
                auto.shared.needs_user();
                auto.backoff.reset();
            }
            warn!(?trigger, ?class, %error, "sync: failed");
            host.emit(&SyncEvent::Finished {
                trigger,
                outcome: Outcome::Failed { class, error },
            });
        }
    }
    result
}

/// Where the engine's follow-up after a `trigger` run goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FollowUp {
    /// Request an automatic run with this trigger.
    Queue(SyncTrigger),
    /// Drop it: the run was `Exit`, and no flight may start while the
    /// process shuts down.
    Skip,
}

impl FollowUp {
    fn after(trigger: SyncTrigger) -> Self {
        match trigger {
            // Rows saved during the push: what a `Write` run covers. Never
            // `Manual`, which is the user's and arms no retry.
            SyncTrigger::Manual => Self::Queue(SyncTrigger::Write),
            // A flight spawned now would race the process exit.
            SyncTrigger::Exit => Self::Skip,
            SyncTrigger::Auto
            | SyncTrigger::Write
            | SyncTrigger::Flush
            | SyncTrigger::Retry
            | SyncTrigger::Online
            | SyncTrigger::Background
            | SyncTrigger::Settings => Self::Queue(trigger),
        }
    }
}

/// The engine asked for one more run after a successful `trigger` run.
fn queue_follow_up<H: Host>(host: &H, trigger: SyncTrigger) {
    match FollowUp::after(trigger) {
        FollowUp::Queue(next) => {
            info!(
                ?trigger,
                ?next,
                "sync: the engine asked for a follow-up run"
            );
            request(host, next);
        }
        FollowUp::Skip => {
            info!(?trigger, "sync: follow-up skipped; the process is exiting");
        }
    }
}

/// The scheduler, if automatic runs may start now.
pub(crate) fn active<H: Host>(host: &H) -> Option<&AutoSync> {
    host.autosync().filter(|auto| auto.active())
}

/// Start an automatic run, or join the one queued behind the current run.
pub(crate) fn request<H: Host>(host: &H, trigger: SyncTrigger) {
    let Some(auto) = active(host) else { return };
    let Admission::Start(flight) = auto.admit(trigger) else {
        info!(?trigger, "sync: queued behind the current run");
        return;
    };
    auto.rt.spawn(drive(host.clone(), flight, trigger));
}

/// Run `trigger`, then whatever queues behind it, until the flight ends.
async fn drive<H: Host>(host: H, mut flight: Flight, mut trigger: SyncTrigger) {
    loop {
        // `run` already logs and emits the outcome.
        if let Err(error) = run(&host, trigger).await
            && trigger.automatic()
            && host.class(&error) == FailureClass::ClearsByItself
        {
            arm_retry(&host);
        }
        match flight.next_queued() {
            Some(next) => trigger = next,
            None => break,
        }
    }
}

#[cfg(test)]
mod tests;
