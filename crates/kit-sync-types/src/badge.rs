//! The dock / Settings sync mark (from OB `src/ctx.rs`).

use std::num::NonZeroU8;

use crate::failure::{FailureClass, FailureKind};

/// The dock / Settings sync mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SyncBadge {
    Off,
    Synced,
    Pending,
    Syncing,
    /// The last run failed with an error that clears by itself, local
    /// changes are still pending, and the streak reached the policy.
    Failed,
    /// The last run failed with an error that needs the user.
    NeedsAction,
}

/// When an ordinary failure starts to show as [`SyncBadge::Failed`]. Until
/// then it stays [`SyncBadge::Pending`], so one blip offline is silent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadgePolicy {
    /// Consecutive [`FailureClass::ClearsByItself`] failures before the warning.
    pub warn_after: NonZeroU8,
}

/// What [`badge`] reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeInput {
    /// Sync is not configured on this device.
    Off,
    On(OnState),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnState {
    pub activity: Activity,
    pub queue: Queue,
    pub last: LastRun,
}

/// Whether a run is in flight (any trigger).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    Idle,
    Running,
}

/// Whether local changes wait to be pushed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Queue {
    Empty,
    Pending,
}

/// How the last finished run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LastRun {
    /// No run yet, or it synced.
    Clean,
    Failed {
        class: FailureClass,
        /// Consecutive failed runs, this one included.
        streak: u32,
    },
}

impl LastRun {
    /// A failed run of the app's own `kind`.
    pub fn failed(kind: impl FailureKind, streak: u32) -> Self {
        Self::Failed {
            class: kind.class(),
            streak,
        }
    }
}

/// Derive the badge. A running Sync wins; a failure that needs the user
/// shows at once; an ordinary one only while changes are pending and after
/// `policy.warn_after` in a row.
pub fn badge(input: BadgeInput, policy: BadgePolicy) -> SyncBadge {
    let BadgeInput::On(state) = input else {
        return SyncBadge::Off;
    };
    if state.activity == Activity::Running {
        return SyncBadge::Syncing;
    }
    let pending = state.queue == Queue::Pending;
    match state.last {
        LastRun::Failed {
            class: FailureClass::NeedsUser,
            ..
        } => SyncBadge::NeedsAction,
        LastRun::Failed {
            class: FailureClass::ClearsByItself,
            streak,
        } if pending && streak >= u32::from(policy.warn_after.get()) => SyncBadge::Failed,
        LastRun::Clean | LastRun::Failed { .. } if pending => SyncBadge::Pending,
        LastRun::Clean | LastRun::Failed { .. } => SyncBadge::Synced,
    }
}

#[cfg(test)]
mod tests;
