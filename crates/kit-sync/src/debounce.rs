//! `Auto` debounce (resume, first load) and the `Online` trigger.

use std::sync::{Mutex, Once};
use std::time::Duration;

use kit_sync_types::SyncTrigger;
use tokio::time::Instant;
use tracing::info;

use crate::backoff::arm_retry;
use crate::host::Host;
use crate::lock;
use crate::run::{active, request};

/// When the last `Auto` ran, and whether the first load happened.
#[derive(Debug)]
pub(crate) struct Debounce {
    last_auto: Mutex<Option<Instant>>,
    first_load: Once,
}

impl Default for Debounce {
    fn default() -> Self {
        Self {
            last_auto: Mutex::new(None),
            first_load: Once::new(),
        }
    }
}

impl Debounce {
    /// True (and restarts the window) when no `Auto` ran within `window`.
    fn due(&self, window: Duration) -> bool {
        let now = Instant::now();
        let mut last = lock(&self.last_auto);
        match *last {
            Some(at) if now.duration_since(at) < window => false,
            Some(_) | None => {
                *last = Some(now);
                true
            }
        }
    }

    /// Forget when the last `Auto` ran. The next `due` admits one.
    pub(crate) fn end_window(&self) {
        *lock(&self.last_auto) = None;
    }
}

/// The app came back to the foreground (Android `Resumed`, first load):
/// `Auto`, debounced. A suspend while automatic runs were active ends that
/// window, so the first resume after it runs. Bursts that never suspend stay
/// collapsed for [`crate::SyncPolicy::auto_debounce`]. Restarts the backoff;
/// when `Auto` is debounced, a stored retry (even one that already fired)
/// comes back at the first step.
pub fn on_resumed<H: Host>(host: &H) {
    let Some(auto) = active(host) else { return };
    let retrying = auto.backoff.retrying();
    auto.backoff.reset();
    if auto.debounce.due(auto.policy.auto_debounce) {
        request(host, SyncTrigger::Auto);
    } else if retrying {
        arm_retry(host);
    }
}

/// The webview reports the network is back: restart the backoff and run now
/// if anything is pending. Not debounced.
pub fn on_online<H: Host>(host: &H) {
    info!("sync: webview online");
    let Some(auto) = active(host) else { return };
    auto.backoff.reset();
    let host = host.clone();
    auto.rt.spawn(async move {
        // A failed count syncs anyway.
        if !matches!(host.pending().await, Ok(0)) {
            request(&host, SyncTrigger::Online);
        }
    });
}

/// First page load: [`on_resumed`], once per [`crate::AutoSync`].
pub fn on_first_load<H: Host>(host: &H) {
    if let Some(auto) = host.autosync() {
        auto.debounce.first_load.call_once(|| on_resumed(host));
    }
}
