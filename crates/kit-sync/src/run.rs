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
                info!(?trigger, "sync: the engine asked for a follow-up run");
                request(host, trigger);
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
