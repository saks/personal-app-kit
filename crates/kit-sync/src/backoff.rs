//! Retry with backoff after failed automatic runs, with jitter.

use std::hash::{BuildHasher, Hasher, RandomState};
use std::num::NonZeroU32;
use std::sync::Mutex;

use kit_sync_types::SyncTrigger;
use tokio::task::AbortHandle;
use tracing::info;

use crate::host::Host;
use crate::lock;
use crate::run::request;

/// A signed fraction of a retry delay to add or remove, within
/// ±[`Spread::MAX`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jitter(f64);

impl Jitter {
    /// No change to the delay.
    pub const NONE: Self = Self(0.0);

    /// Clamped to ±[`Spread::MAX`] (±50 %); NaN is no jitter. This is only
    /// the outer limit: [`crate::SyncPolicy::retry_delay`] then bounds it to
    /// the policy's spread ([`Spread::DEFAULT`], ±20 %, unless changed).
    pub fn new(fraction: f64) -> Self {
        if fraction.is_nan() {
            Self::NONE
        } else {
            Self(fraction.clamp(-Spread::MAX.0, Spread::MAX.0))
        }
    }

    pub(crate) fn fraction(self) -> f64 {
        self.0
    }
}

/// How far [`Jitter`] may move a retry delay: ±this fraction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spread(f64);

impl Spread {
    /// No jitter at all (deterministic delays).
    pub const NONE: Self = Self(0.0);
    /// ±20 %.
    pub const DEFAULT: Self = Self(0.2);
    /// The widest spread, so a delay never shrinks below half.
    pub const MAX: Self = Self(0.5);

    /// Clamped to `0..=`[`Spread::MAX`]; NaN is no spread.
    pub fn new(fraction: f64) -> Self {
        if fraction.is_nan() {
            Self::NONE
        } else {
            Self(fraction.clamp(0.0, Self::MAX.0))
        }
    }

    /// `jitter` limited to ±this spread.
    pub fn bound(self, jitter: Jitter) -> Jitter {
        Jitter(jitter.0.clamp(-self.0, self.0))
    }

    /// A uniformly random jitter within ±this spread.
    pub(crate) fn sample(self) -> Jitter {
        Jitter((random_unit() * 2.0 - 1.0) * self.0)
    }
}

/// A random number in `[0, 1)` from std's per-process random hash keys, so no
/// new dependency. Good enough to spread retries; not for anything secret.
fn random_unit() -> f64 {
    let bits = RandomState::new().build_hasher().finish() >> 11;
    bits as f64 / (1u64 << 53) as f64
}

/// The backoff after automatic failures that clear by themselves.
#[derive(Debug)]
enum Retry {
    Off,
    /// A count is stored. The timer may still be waiting, or it may already
    /// have fired (`AbortHandle::is_finished`); the count remains either way.
    Armed {
        failures: NonZeroU32,
        task: AbortHandle,
    },
}

/// A backoff event. The caller decides whether it happened. `step` decides
/// what it does to the stored count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    /// A local save, while automatic runs are active.
    Write,
    /// The network is back, while automatic runs are active.
    Online,
    /// Settings were saved. Clears even when automatic runs are inactive.
    SettingsSaved,
    /// A successful run whose epoch is still current.
    Succeeded,
    /// A `NeedsUser` failure whose epoch is still current.
    NeedsUser,
    /// An automatic failure that clears by itself, whose epoch is still
    /// current.
    Failed,
    /// The app resumed. `debounced` means this resume will not run `Auto`.
    Resumed { debounced: bool },
}

impl Event {
    /// These events are ignored when automatic runs are inactive.
    fn requires_active(self) -> bool {
        matches!(
            self,
            Self::Write | Self::Online | Self::Failed | Self::Resumed { .. }
        )
    }
}

/// What one event does to the stored count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    /// Leave the count and the timer alone.
    Keep,
    /// Drop the count and abort the timer, if there is one.
    Clear,
    /// Abort any current timer and arm a new one at this count.
    Arm(NonZeroU32),
}

/// What `event` does to a stored count. `None` is off. `Some` is a stored
/// count, including a timer that has already fired.
fn step(stored: Option<NonZeroU32>, event: Event) -> Action {
    match event {
        Event::Failed => Action::Arm(match stored {
            None => NonZeroU32::MIN,
            Some(failures) => failures.saturating_add(1),
        }),
        Event::Resumed { debounced: true } => match stored {
            None => Action::Keep,
            Some(_) => Action::Arm(NonZeroU32::MIN),
        },
        Event::Write
        | Event::Online
        | Event::SettingsSaved
        | Event::Succeeded
        | Event::NeedsUser
        | Event::Resumed { debounced: false } => Action::Clear,
    }
}

/// The armed retry, if any.
#[derive(Debug)]
pub(crate) struct Backoff(Mutex<Retry>);

impl Default for Backoff {
    fn default() -> Self {
        Self(Mutex::new(Retry::Off))
    }
}

impl Backoff {
    pub(crate) fn backoff_attempts(&self) -> u32 {
        match &*lock(&self.0) {
            Retry::Off => 0,
            Retry::Armed { failures, .. } => failures.get(),
        }
    }

    /// A retry is armed and has not fired yet.
    pub(crate) fn armed(&self) -> bool {
        match &*lock(&self.0) {
            Retry::Off => false,
            Retry::Armed { task, .. } => !task.is_finished(),
        }
    }

    /// Apply `event` under this lock. `arm` runs only when the decision arms
    /// a timer, and it must not call [`apply`]: the lock is held across it.
    fn transition(&self, event: Event, arm: impl FnOnce(NonZeroU32) -> AbortHandle) {
        let mut retry = lock(&self.0);
        let stored = match &*retry {
            Retry::Off => None,
            Retry::Armed { failures, .. } => Some(*failures),
        };
        match step(stored, event) {
            Action::Keep => {}
            Action::Clear => {
                if let Retry::Armed { task, .. } = std::mem::replace(&mut *retry, Retry::Off) {
                    task.abort();
                }
            }
            Action::Arm(failures) => {
                if let Retry::Armed { task, .. } = std::mem::replace(&mut *retry, Retry::Off) {
                    task.abort();
                }
                let task = arm(failures);
                *retry = Retry::Armed { failures, task };
            }
        }
    }
}

/// Apply one [`Event`] under the backoff lock. This is the only mutator.
///
/// `Write`, `Online`, `Failed`, and `Resumed` do nothing when automatic runs
/// are inactive. `SettingsSaved`, `Succeeded`, and `NeedsUser` still clear.
/// No scheduler at all does nothing.
pub(crate) fn apply<H: Host>(host: &H, event: Event) {
    let Some(auto) = host.autosync() else { return };
    if event.requires_active() && !auto.active() {
        return;
    }
    let task_host = host.clone();
    let rt = auto.rt.clone();
    let policy = auto.policy.clone();
    auto.backoff.transition(event, move |failures| {
        let wait = policy.retry_delay(failures, policy.jitter.sample());
        info!(
            failures = failures.get(),
            wait_ms = wait.as_millis(),
            "sync: retry armed"
        );
        // One `Retry` run after the delay. Nothing pending skips the run (no
        // polling). Dropping the `JoinHandle` detaches that task; the stored
        // `AbortHandle` is what cancels it.
        rt.spawn(async move {
            tokio::time::sleep(wait).await;
            if matches!(task_host.pending().await, Ok(0)) {
                info!("sync: retry skipped; nothing pending");
                return;
            }
            request(&task_host, SyncTrigger::Retry);
        })
        .abort_handle()
    });
}

#[cfg(test)]
mod tests;
