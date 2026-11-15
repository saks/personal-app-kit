//! The fake [`Host`]: a scripted engine on a paused clock (OB's `Fake`).

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kit_sync_types::{FailureClass, SyncTrigger};
use tokio::runtime::Handle;
use tokio::sync::Mutex as AsyncMutex;

use crate::{AutoSync, Gate, Host, Mode, Outcome, Spread, SyncEvent, SyncPolicy, lock};

#[derive(Clone, Copy, Debug)]
pub(super) enum Behavior {
    Succeed {
        takes: Duration,
    },
    /// The server rejects the token: `NeedsUser`.
    RejectToken,
    /// Offline / 5xx: clears by itself.
    Offline,
    Hang,
    Panic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FakeError {
    RejectToken,
    Offline,
}

impl fmt::Display for FakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::RejectToken => "status=401 Unauthorized",
            Self::Offline => "client error (Connect)",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Report {
    pub(super) local_changes: u64,
    /// Rows the push did not take (ST's `still_queued`).
    pub(super) left_behind: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Seen {
    Started(SyncTrigger),
    Synced(SyncTrigger, Report),
    Failed(SyncTrigger, FailureClass, FakeError),
}

pub(super) struct FakeInner {
    pub(super) auto: AutoSync,
    pub(super) behavior: Mutex<Behavior>,
    /// Behaviours for the next syncs, in order, before `behavior` applies
    /// again: fixed per call, not by when the test changes `behavior`.
    script: Mutex<VecDeque<Behavior>>,
    pub(super) pending: AtomicU64,
    pending_calls: AtomicU64,
    fail_pending: AtomicBool,
    suspended: Mutex<Vec<Result<u64, FakeError>>>,
    /// How many of the next successful runs report rows left behind.
    pub(super) leave_behind: AtomicU64,
    events: Mutex<Vec<Seen>>,
    /// When locked, `pending` and `sync` wait: stands in for the engine lock.
    pub(super) hold: Arc<AsyncMutex<()>>,
}

#[derive(Clone)]
pub(super) struct Fake(pub(super) Arc<FakeInner>);

/// Default timings without jitter, so retry times are exact.
pub(super) fn exact_policy() -> SyncPolicy {
    SyncPolicy {
        jitter: Spread::NONE,
        ..SyncPolicy::default()
    }
}

impl Fake {
    pub(super) fn with_policy(
        policy: SyncPolicy,
        mode: Mode,
        gate: Gate,
        behavior: Behavior,
    ) -> Self {
        Self(Arc::new(FakeInner {
            auto: AutoSync::new(policy, mode, gate, Handle::current()),
            behavior: Mutex::new(behavior),
            script: Mutex::new(VecDeque::new()),
            pending: AtomicU64::new(1),
            pending_calls: AtomicU64::new(0),
            fail_pending: AtomicBool::new(false),
            suspended: Mutex::new(Vec::new()),
            leave_behind: AtomicU64::new(0),
            events: Mutex::new(Vec::new()),
            hold: Arc::new(AsyncMutex::new(())),
        }))
    }

    pub(super) fn new(mode: Mode, gate: Gate, behavior: Behavior) -> Self {
        Self::with_policy(exact_policy(), mode, gate, behavior)
    }

    pub(super) fn ready() -> Self {
        Self::new(
            Mode::On,
            Gate::Ready,
            Behavior::Succeed {
                takes: Duration::from_millis(100),
            },
        )
    }

    pub(super) fn auto(&self) -> &AutoSync {
        &self.0.auto
    }

    pub(super) fn behave(&self, behavior: Behavior) {
        *lock(&self.0.behavior) = behavior;
    }

    /// The next syncs behave like `behaviors`, in order, whichever task
    /// runs them and whenever it gets to run.
    pub(super) fn script(&self, behaviors: impl IntoIterator<Item = Behavior>) {
        lock(&self.0.script).extend(behaviors);
    }

    pub(super) fn set_pending(&self, rows: u64) {
        self.0.pending.store(rows, Ordering::SeqCst);
    }

    pub(super) fn pending_now(&self) -> u64 {
        self.0.pending.load(Ordering::SeqCst)
    }

    pub(super) fn pending_calls(&self) -> u64 {
        self.0.pending_calls.load(Ordering::SeqCst)
    }

    pub(super) fn fail_pending(&self) {
        self.0.fail_pending.store(true, Ordering::SeqCst);
    }

    pub(super) fn suspended_calls(&self) -> Vec<Result<u64, FakeError>> {
        lock(&self.0.suspended).clone()
    }

    pub(super) fn started(&self) -> Vec<SyncTrigger> {
        lock(&self.0.events)
            .iter()
            .filter_map(|seen| match seen {
                Seen::Started(trigger) => Some(*trigger),
                Seen::Synced(..) | Seen::Failed(..) => None,
            })
            .collect()
    }

    pub(super) fn finished(&self) -> Vec<Seen> {
        lock(&self.0.events)
            .iter()
            .filter(|seen| !matches!(seen, Seen::Started(_)))
            .cloned()
            .collect()
    }
}

impl Host for Fake {
    type Report = Report;
    type Error = FakeError;

    fn autosync(&self) -> Option<&AutoSync> {
        Some(&self.0.auto)
    }

    async fn pending(&self) -> Result<u64, FakeError> {
        let _hold = self.0.hold.lock().await;
        self.0.pending_calls.fetch_add(1, Ordering::SeqCst);
        if self.0.fail_pending.load(Ordering::SeqCst) {
            return Err(FakeError::Offline);
        }
        Ok(self.0.pending.load(Ordering::SeqCst))
    }

    async fn sync(&self) -> Result<Report, FakeError> {
        let _hold = self.0.hold.lock().await;
        let scripted = lock(&self.0.script).pop_front();
        let behavior = scripted.unwrap_or_else(|| *lock(&self.0.behavior));
        match behavior {
            Behavior::Succeed { takes } => {
                // Counted at the start, like the engines do.
                let local_changes = self.0.pending.swap(0, Ordering::SeqCst);
                tokio::time::sleep(takes).await;
                // One task runs at a time on the test runtime.
                let left_behind = u64::from(self.0.leave_behind.load(Ordering::SeqCst) > 0);
                self.0.leave_behind.fetch_sub(left_behind, Ordering::SeqCst);
                Ok(Report {
                    local_changes,
                    left_behind,
                })
            }
            Behavior::RejectToken => Err(FakeError::RejectToken),
            Behavior::Offline => Err(FakeError::Offline),
            Behavior::Hang => std::future::pending().await,
            Behavior::Panic => panic!("the engine panicked"),
        }
    }

    fn emit(&self, event: &SyncEvent<'_, Report, FakeError>) {
        let seen = match event {
            SyncEvent::Started { trigger } => Seen::Started(*trigger),
            SyncEvent::Finished {
                trigger,
                outcome: Outcome::Synced(report),
            } => Seen::Synced(*trigger, **report),
            SyncEvent::Finished {
                trigger,
                outcome: Outcome::Failed { class, error },
            } => Seen::Failed(*trigger, *class, **error),
        };
        lock(&self.0.events).push(seen);
    }

    fn class(&self, error: &FakeError) -> FailureClass {
        match error {
            FakeError::RejectToken => FailureClass::NeedsUser,
            FakeError::Offline => FailureClass::ClearsByItself,
        }
    }

    fn wants_follow_up(&self, report: &Report) -> bool {
        report.left_behind > 0
    }

    fn suspended(&self, pending: Result<u64, &FakeError>) {
        lock(&self.0.suspended).push(pending.map_err(|error| *error));
    }
}

pub(super) async fn advance(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

pub(super) fn offline() -> Fake {
    Fake::new(Mode::On, Gate::Ready, Behavior::Offline)
}
