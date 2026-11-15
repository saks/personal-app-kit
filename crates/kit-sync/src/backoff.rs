//! Retry with backoff after failed automatic runs, with jitter.

use std::hash::{BuildHasher, Hasher, RandomState};
use std::num::NonZeroU32;
use std::sync::Mutex;

use kit_sync_types::SyncTrigger;
use tokio::task::AbortHandle;
use tracing::info;

use crate::host::Host;
use crate::lock;
use crate::run::{active, request};

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
    Armed {
        failures: NonZeroU32,
        task: AbortHandle,
    },
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
    /// Drop the backoff: the next failure waits the first step again.
    pub(crate) fn reset(&self) {
        if let Retry::Armed { task, .. } = std::mem::replace(&mut *lock(&self.0), Retry::Off) {
            task.abort();
        }
    }

    pub(crate) fn backoff_attempts(&self) -> u32 {
        match &*lock(&self.0) {
            Retry::Off => 0,
            Retry::Armed { failures, .. } => failures.get(),
        }
    }

    /// Failing, whether or not the retry already fired.
    pub(crate) fn retrying(&self) -> bool {
        matches!(&*lock(&self.0), Retry::Armed { .. })
    }

    /// A retry is armed and has not fired yet.
    pub(crate) fn armed(&self) -> bool {
        match &*lock(&self.0) {
            Retry::Off => false,
            Retry::Armed { task, .. } => !task.is_finished(),
        }
    }
}

/// After an automatic failure that clears by itself: one `Retry` run after
/// the next delay. The retry skips the run when nothing is pending (no
/// polling).
pub(crate) fn arm_retry<H: Host>(host: &H) {
    let Some(auto) = active(host) else { return };
    let mut retry = lock(&auto.backoff.0);
    let failures = match &*retry {
        Retry::Off => NonZeroU32::MIN,
        Retry::Armed { failures, task } => {
            task.abort();
            failures.saturating_add(1)
        }
    };
    let wait = auto
        .policy
        .retry_delay(failures, auto.policy.jitter.sample());
    info!(
        failures = failures.get(),
        wait_ms = wait.as_millis(),
        "sync: retry armed"
    );
    let task_host = host.clone();
    let task = auto.rt.spawn(async move {
        tokio::time::sleep(wait).await;
        if matches!(task_host.pending().await, Ok(0)) {
            info!("sync: retry skipped; nothing pending");
            return;
        }
        request(&task_host, SyncTrigger::Retry);
    });
    *retry = Retry::Armed {
        failures,
        task: task.abort_handle(),
    };
}

#[cfg(test)]
mod tests;
