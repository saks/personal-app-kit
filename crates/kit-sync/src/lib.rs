//! Engine-agnostic sync scheduler shared by both apps (sickness_tracker#95).
//!
//! A port of octo-budget-rs `autosync` (main `2ebfd24`) with the app parts
//! left out, plus four pieces from sickness_tracker: the follow-up cap, a
//! flight that ends on drop or panic, an engine-requested follow-up, and the
//! public `failures` / `scheduled` queries.
//!
//! - `AutoSync`: the scheduler state one app holds (single flight, write
//!   timer, debounce, backoff, gate) and the `SyncPolicy` it runs with.
//! - `Host`: what the app supplies (pending count, one Sync, events, and the
//!   retry rule as a `FailureClass`).
//! - `run`: the only way a Sync runs. Manual calls it directly; every
//!   automatic trigger goes through the single flight.
//! - Entry points for the app's events: `schedule_after_write`,
//!   `on_suspended`, `on_resumed`, `on_first_load`, `on_online`,
//!   `settings_saved`, `flush_before_exit`.
//! - `with_budget`: a time budget for one push or pull, as `BudgetExceeded`.
//!
//! Plain tokio (`Handle` + `AbortHandle`): no Tauri, no turso. The kit never
//! holds a lock across `Host::sync`; the engine decides what it locks.
//!
//! Host only: on `wasm32` this crate is empty, so these names are plain text
//! rather than intra-doc links (they would not resolve there).
#![cfg(not(target_arch = "wasm32"))]
#![warn(missing_docs)]

mod backoff;
mod budget;
mod debounce;
mod flight;
mod gate;
mod host;
mod policy;
mod run;
mod write_timer;

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use kit_sync_types::SyncTrigger;
use tokio::runtime::Handle;

pub use backoff::{Jitter, Spread};
pub use budget::{BudgetExceeded, with_budget};
pub use debounce::{on_first_load, on_online, on_resumed};
pub use flight::{Admission, Flight};
pub use gate::{Gate, Mode, settings_saved};
pub use host::{Host, Outcome, SyncEvent};
pub use policy::{RetrySteps, SyncPolicy};
pub use run::run;
pub use write_timer::{flush_before_exit, on_suspended, schedule_after_write};

use backoff::Backoff;
use debounce::Debounce;
use flight::Shared;
use write_timer::WriteTimer;

/// The scheduler state of one app process. The app keeps it next to its
/// engine and hands it out through [`Host::autosync`].
#[derive(Debug)]
pub struct AutoSync {
    policy: SyncPolicy,
    rt: Handle,
    shared: Arc<Shared>,
    write_timer: WriteTimer,
    debounce: Debounce,
    backoff: Backoff,
}

impl AutoSync {
    /// A scheduler that spawns its timers and runs on `rt`.
    pub fn new(policy: SyncPolicy, mode: Mode, gate: Gate, rt: Handle) -> Self {
        Self {
            shared: Arc::new(Shared::new(mode, gate, policy.max_follow_ups)),
            policy,
            rt,
            write_timer: WriteTimer::default(),
            debounce: Debounce::default(),
            backoff: Backoff::default(),
        }
    }

    /// The policy this scheduler runs with (engines read the push and pull
    /// budgets from it).
    pub fn policy(&self) -> &SyncPolicy {
        &self.policy
    }

    /// Whether automatic runs may start now: [`Mode::On`] and [`Gate::Ready`].
    pub fn active(&self) -> bool {
        self.shared.active()
    }

    /// Enter the single flight: start it, or join the one run queued behind
    /// the current run. A background job (Android WorkManager) calls this
    /// itself, runs [`run`] while it holds the [`Flight`], then drains
    /// [`Flight::next_queued`].
    pub fn admit(&self, trigger: SyncTrigger) -> Admission {
        Shared::admit(&self.shared, trigger)
    }

    /// Automatic failures in a row behind the armed backoff; 0 when none.
    pub fn failures(&self) -> u32 {
        self.backoff.failures()
    }

    /// The host will sync by itself: a write timer or a retry is armed and
    /// has not fired yet.
    pub fn scheduled(&self) -> bool {
        self.write_timer.armed() || self.backoff.armed()
    }
}

/// Lock a std mutex, recovering the data if a holder panicked.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
