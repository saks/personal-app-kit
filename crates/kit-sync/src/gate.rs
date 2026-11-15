//! Whether automatic runs may start: the build's [`Mode`] and the
//! settings' [`Gate`].

use kit_sync_types::SyncTrigger;

use crate::host::Host;
use crate::run::request;

/// Whether this process schedules automatic runs at all. The app decides
/// (for example from a cargo feature).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Automatic runs as usual.
    On,
    /// Only Manual runs (an e2e build).
    Off,
}

/// Whether automatic runs may start right now. The app derives it from its
/// own settings and passes it in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// Sync is not set up on this device.
    Unconfigured,
    /// Set up and working (as far as we know).
    Ready,
    /// The last run failed in a way only the user can fix. Cleared by
    /// [`settings_saved`] or a successful run.
    NeedsUser,
}

/// The app saved its settings: take the new `gate`, clear `NeedsUser`, drop
/// any armed retry, and start a `Settings` run if Sync stays on and anything
/// is pending. Spawns, so the run waits for a lock the caller may still hold
/// instead of deadlocking.
pub fn settings_saved<H: Host>(host: &H, gate: Gate) {
    let Some(auto) = host.autosync() else { return };
    auto.shared.set_gate(gate);
    auto.backoff.reset();
    if !auto.active() {
        return;
    }
    let host = host.clone();
    auto.rt.spawn(async move {
        // A failed count syncs anyway.
        if !matches!(host.pending().await, Ok(0)) {
            request(&host, SyncTrigger::Settings);
        }
    });
}
