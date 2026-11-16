//! The single flight: one automatic run at a time, one more queued behind
//! it, and a [`Flight`] guard that ends the flight on drop, panic or abort.

use std::sync::{Arc, Mutex};

use kit_sync_types::SyncTrigger;
use tracing::warn;

use crate::gate::{Gate, Mode};
use crate::lock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Idle,
    Running,
    /// One more run waits for the current one. Later requests join it.
    Queued(SyncTrigger),
}

/// Gate plus the generation of the last change. One mutex so a run can
/// apply its outcome only when the generation it recorded is still current.
#[derive(Debug)]
struct GateState {
    gate: Gate,
    /// Bumped by [`Shared::set_gate`] and by a gate transition from a run. A
    /// run records it before `sync` and applies its outcome only when it
    /// still matches.
    epoch: u64,
}

/// Flight state shared with the [`Flight`] guard, which outlives the borrow
/// of [`crate::AutoSync`] (it moves into the spawned run).
#[derive(Debug)]
pub(crate) struct Shared {
    mode: Mode,
    gate: Mutex<GateState>,
    slot: Mutex<Slot>,
    max_follow_ups: u32,
}

impl Shared {
    pub(crate) fn new(mode: Mode, gate: Gate, max_follow_ups: u32) -> Self {
        Self {
            mode,
            gate: Mutex::new(GateState { gate, epoch: 0 }),
            slot: Mutex::new(Slot::Idle),
            max_follow_ups,
        }
    }

    pub(crate) fn active(&self) -> bool {
        self.mode == Mode::On && lock(&self.gate).gate == Gate::Ready
    }

    pub(crate) fn epoch(&self) -> u64 {
        lock(&self.gate).epoch
    }

    /// Store `gate` and bump the epoch, even when the value is unchanged: a
    /// settings save is a new generation (`Ready` → `Ready` still invalidates
    /// an in-flight run).
    pub(crate) fn set_gate(&self, gate: Gate) {
        let mut state = lock(&self.gate);
        state.gate = gate;
        state.epoch = state.epoch.wrapping_add(1);
    }

    /// `NeedsUser` back to `Ready` when `seen` is still current. Returns
    /// whether `seen` matches; bumps the epoch only when the gate changes.
    pub(crate) fn clear_needs_user(&self, seen: u64) -> bool {
        let mut state = lock(&self.gate);
        if state.epoch != seen {
            return false;
        }
        if state.gate == Gate::NeedsUser {
            state.gate = Gate::Ready;
            state.epoch = state.epoch.wrapping_add(1);
        }
        true
    }

    /// `Ready` to `NeedsUser` when `seen` is still current. Returns whether
    /// `seen` matches; bumps the epoch only when the gate changes.
    pub(crate) fn needs_user(&self, seen: u64) -> bool {
        let mut state = lock(&self.gate);
        if state.epoch != seen {
            return false;
        }
        if state.gate == Gate::Ready {
            state.gate = Gate::NeedsUser;
            state.epoch = state.epoch.wrapping_add(1);
        }
        true
    }

    pub(crate) fn admit(shared: &Arc<Self>, trigger: SyncTrigger) -> Admission {
        let mut slot = lock(&shared.slot);
        match *slot {
            Slot::Idle => {
                *slot = Slot::Running;
                Admission::Start(Flight {
                    shared: Some(Arc::clone(shared)),
                    follow_ups: 0,
                })
            }
            Slot::Running => {
                *slot = Slot::Queued(trigger);
                Admission::Joined
            }
            Slot::Queued(_) => Admission::Joined,
        }
    }

    /// Whether no flight is running or queued (tests).
    #[cfg(test)]
    pub(crate) fn idle(&self) -> bool {
        *lock(&self.slot) == Slot::Idle
    }
}

/// The answer to [`crate::AutoSync::admit`].
///
/// Dropping a started [`Flight`] ends it and drops any queued run. See
/// [Dropping](Flight#dropping).
#[derive(Debug)]
#[must_use = "dropping a started Flight ends the flight and drops any queued run"]
pub enum Admission {
    /// The flight was idle: the caller owns it now and runs.
    Start(Flight),
    /// A run is in flight: one run is queued behind it (or was already).
    Joined,
}

impl Admission {
    /// The [`Flight`] when this call started it. `None` when a run is already
    /// in flight (this trigger joined the queued one).
    pub fn into_flight(self) -> Option<Flight> {
        match self {
            Self::Start(flight) => Some(flight),
            Self::Joined => None,
        }
    }
}

/// Ownership of the running flight.
///
/// # Dropping
///
/// Dropping a `Flight` (a panic, an aborted task, an early return, `let _ =`,
/// or `matches!(…, Start(_))`, which looks like a check) sets the flight idle
/// and drops the queued trigger. A background job holds
/// the `Flight` across [`crate::run`] and [`Flight::next_queued`], and keeps
/// calling `next_queued` after a failed run:
///
/// ```no_run
/// # use kit_sync::{AutoSync, Host, run};
/// # use kit_sync_types::SyncTrigger;
/// # async fn job<H: Host>(host: &H, auto: &AutoSync, mut trigger: SyncTrigger) {
/// let Some(mut flight) = auto.admit(trigger).into_flight() else {
///     return;
/// };
/// loop {
///     // `run` already emits the outcome. Returning here would drop `flight`.
///     let _ = run(host, trigger).await;
///     match flight.next_queued() {
///         Some(next) => trigger = next,
///         None => break,
///     }
/// }
/// # }
/// ```
#[derive(Debug)]
#[must_use = "dropping a Flight ends the flight and drops any queued run"]
pub struct Flight {
    /// `None` once the flight has ended.
    shared: Option<Arc<Shared>>,
    follow_ups: u32,
}

impl Flight {
    /// Call when a run ends: the queued trigger to run next, keeping the
    /// flight, or `None`, ending it. Ends the flight when automatic runs are
    /// off or after [`crate::SyncPolicy::max_follow_ups`] queued runs; the
    /// queued trigger is then dropped until the next event.
    pub fn next_queued(&mut self) -> Option<SyncTrigger> {
        let shared = self.shared.take()?;
        let next = {
            let mut slot = lock(&shared.slot);
            let next = match *slot {
                Slot::Queued(trigger) if shared.active() => self.follow_up(&shared, trigger),
                Slot::Idle | Slot::Running | Slot::Queued(_) => None,
            };
            *slot = match next {
                Some(_) => Slot::Running,
                None => Slot::Idle,
            };
            next
        };
        if next.is_some() {
            self.shared = Some(shared);
        }
        next
    }

    /// Count one more follow-up, unless the cap is reached.
    fn follow_up(&mut self, shared: &Shared, trigger: SyncTrigger) -> Option<SyncTrigger> {
        if self.follow_ups >= shared.max_follow_ups {
            warn!(
                follow_ups = self.follow_ups,
                ?trigger,
                "sync: follow-up limit reached"
            );
            return None;
        }
        self.follow_ups += 1;
        Some(trigger)
    }
}

impl Drop for Flight {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.take() {
            *lock(&shared.slot) = Slot::Idle;
        }
    }
}

#[cfg(test)]
mod tests;
