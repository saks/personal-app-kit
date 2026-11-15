//! The timings and limits the scheduler runs with.

use std::num::NonZeroU32;
use std::time::Duration;

use crate::backoff::{Jitter, Spread};

/// Every timing and limit of the scheduler. [`Default`] is what both apps use
/// today.
#[derive(Clone, Debug, PartialEq)]
pub struct SyncPolicy {
    /// Quiet time after the last save before a `Write` run (re-armed by every
    /// save).
    pub write_delay: Duration,
    /// `Auto` runs (resume, first load) at most once per this window, counted
    /// from the last `Auto`.
    pub auto_debounce: Duration,
    /// Retry delays after the 1st, 2nd, … automatic failure in a row.
    pub retry_steps: RetrySteps,
    /// Random spread around each retry delay.
    pub jitter: Spread,
    /// Queued runs one flight may chain before it lets go (only reached if
    /// saves never stop).
    pub max_follow_ups: u32,
    /// The engine's budget for one push (see [`crate::with_budget`]).
    pub push_budget: Duration,
    /// The engine's budget for one pull (see [`crate::with_budget`]).
    pub pull_budget: Duration,
    /// The most [`crate::flush_before_exit`] may take.
    pub exit_budget: Duration,
}

impl Default for SyncPolicy {
    /// 1.5 s, 30 s, 5/15/60/300 s, ±20 %, 5, 15 s, 15 s, 3 s.
    fn default() -> Self {
        Self {
            write_delay: Duration::from_millis(1500),
            auto_debounce: Duration::from_secs(30),
            retry_steps: RetrySteps::default(),
            jitter: Spread::DEFAULT,
            max_follow_ups: 5,
            push_budget: Duration::from_secs(15),
            pull_budget: Duration::from_secs(15),
            exit_budget: Duration::from_secs(3),
        }
    }
}

impl SyncPolicy {
    /// How long to wait before retrying after `failures` automatic failures in
    /// a row: the step for that count (the last step repeats), scaled by
    /// `jitter` bounded to this policy's [`Spread`].
    pub fn retry_delay(&self, failures: NonZeroU32, jitter: Jitter) -> Duration {
        let base = self.retry_steps.step(failures);
        let factor = 1.0 + self.jitter.bound(jitter).fraction();
        Duration::try_from_secs_f64(base.as_secs_f64() * factor).unwrap_or(base)
    }
}

/// Retry delays, at least one. The last one repeats.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetrySteps {
    first: Duration,
    rest: Vec<Duration>,
}

impl RetrySteps {
    /// `first` after the first failure, then `rest` in order; the last step
    /// repeats.
    pub fn new(first: Duration, rest: impl IntoIterator<Item = Duration>) -> Self {
        Self {
            first,
            rest: rest.into_iter().collect(),
        }
    }

    /// The step after `failures` failures in a row.
    pub fn step(&self, failures: NonZeroU32) -> Duration {
        let later = usize::try_from(failures.get() - 1).unwrap_or(usize::MAX);
        match later.checked_sub(1) {
            None => self.first,
            Some(index) => self
                .rest
                .get(index)
                .or(self.rest.last())
                .copied()
                .unwrap_or(self.first),
        }
    }
}

impl Default for RetrySteps {
    /// 5 s → 15 s → 60 s → 5 min.
    fn default() -> Self {
        Self::new(
            Duration::from_secs(5),
            [
                Duration::from_secs(15),
                Duration::from_secs(60),
                Duration::from_secs(300),
            ],
        )
    }
}

#[cfg(test)]
mod tests;
