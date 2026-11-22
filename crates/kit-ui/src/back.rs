//! Android Back while an overlay host is mounted.
//!
//! The plugin listener exists only for that lifetime. A press becomes the
//! topmost overlay's Escape. Unregister puts the next press back on the
//! activity default. A press that lands in that gap, with no WebView history,
//! is ignored: `app.exit` is `finishAffinity`, and the webview has no
//! `core:app:allow-exit` permission.

use std::cell::{Cell, RefCell};

use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{KeyboardEvent, KeyboardEventInit};

use crate::overlay::{OVERLAY_HOST_ATTR, OVERLAY_ROOT_ID};
use crate::warn;

/// Failed `onBackButtonPress` attempts before giving up while an overlay is open.
const REGISTER_ATTEMPTS: u32 = 3;

#[wasm_bindgen]
extern "C" {
    /// Tauri's Android `back-button` plugin event. Owning a listener replaces the
    /// activity default for as long as it is registered.
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "app"], js_name = onBackButtonPress, catch)]
    async fn on_back_button_press(handler: &js_sys::Function) -> Result<JsValue, JsValue>;
}

/// How many overlays currently want Android Back.
///
/// `Start` / `Stop` are the edges where the plugin listener is registered or removed.
/// While the count is positive, Back is the topmost overlay's Escape. At zero there
/// is no listener, so Back stays the system default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BackClaim {
    count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackListen {
    Start,
    Hold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackRelease {
    Stop,
    Hold,
}

impl BackClaim {
    const fn new() -> Self {
        Self { count: 0 }
    }

    fn acquire(&mut self) -> BackListen {
        self.count += 1;
        if self.count == 1 {
            BackListen::Start
        } else {
            BackListen::Hold
        }
    }

    fn release(&mut self) -> BackRelease {
        if self.count == 0 {
            return BackRelease::Hold;
        }
        self.count -= 1;
        if self.count == 0 {
            BackRelease::Stop
        } else {
            BackRelease::Hold
        }
    }
}

/// What one Back press does. `Unknown` stays on Escape so a detection failure
/// does not drop the press while a sheet might still be open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OverlayPresence {
    Open,
    Closed,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackAction {
    /// Topmost overlay's Escape (`use_overlay_keys` or the app's own handler).
    Escape,
    /// WebView history, the plugin's `canGoBack` path.
    HistoryBack,
    /// Listener still registered after the last host unmounted, and the webview
    /// has no history. Ignore this press. Unregister restores the plugin default.
    Ignore,
}

fn back_action(overlay: OverlayPresence, can_go_back: bool) -> BackAction {
    match overlay {
        OverlayPresence::Open | OverlayPresence::Unknown => BackAction::Escape,
        OverlayPresence::Closed if can_go_back => BackAction::HistoryBack,
        OverlayPresence::Closed => BackAction::Ignore,
    }
}

/// `failures` counts this failure. Retry while an overlay is still mounted and
/// the attempt budget remains.
const fn should_retry_register(claims: u32, failures: u32) -> bool {
    claims > 0 && failures < REGISTER_ATTEMPTS
}

#[derive(Deserialize)]
struct BackPayload {
    #[serde(rename = "canGoBack")]
    can_go_back: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BackPhase {
    Idle,
    Starting,
    Live,
    Stopping,
}

struct LiveListener {
    listener: JsValue,
    /// Kept so the plugin can keep calling it. Dropped after `unregister`.
    handler: Closure<dyn FnMut(JsValue)>,
}

/// Holds one Android Back claim; dropping it releases the claim. Inactive
/// (no claim) off Android.
#[must_use = "dropping the guard releases Android Back immediately"]
pub struct BackGuard {
    active: bool,
}

impl Drop for BackGuard {
    fn drop(&mut self) {
        if self.active {
            release_android_back();
        }
    }
}

thread_local! {
    static CLAIM: Cell<BackClaim> = const { Cell::new(BackClaim::new()) };
    static PHASE: Cell<BackPhase> = const { Cell::new(BackPhase::Idle) };
    static GEN: Cell<u32> = const { Cell::new(0) };
    static LISTENER: RefCell<Option<LiveListener>> = const { RefCell::new(None) };
    static REGISTER_FAILURES: Cell<u32> = const { Cell::new(0) };
}

fn is_android() -> bool {
    web_sys::window()
        .and_then(|window| window.navigator().user_agent().ok())
        .is_some_and(|agent| agent.contains("Android"))
}

fn claim_count() -> u32 {
    CLAIM.with(|claim| claim.get().count)
}

fn clear_register_failures() {
    REGISTER_FAILURES.with(|slot| slot.set(0));
}

fn record_register_failure() -> u32 {
    REGISTER_FAILURES.with(|slot| {
        let next = slot.get().saturating_add(1);
        slot.set(next);
        next
    })
}

/// Register `back-button` for this overlay's lifetime. Desktop is a no-op: the
/// plugin command exists only on Android, and Escape already covers a keyboard.
pub fn claim_android_back() -> BackGuard {
    if !is_android() {
        return BackGuard { active: false };
    }
    let listen = CLAIM.with(|claim| {
        let mut value = claim.get();
        let listen = value.acquire();
        claim.set(value);
        listen
    });
    let phase = PHASE.with(|phase| phase.get());
    if phase == BackPhase::Idle {
        // A failed register leaves the phase idle while overlays are still
        // mounted. A later claim gets a fresh attempt budget.
        if listen != BackListen::Start {
            clear_register_failures();
        }
        ensure_listener();
    } else if listen == BackListen::Start {
        ensure_listener();
    }
    BackGuard { active: true }
}

fn release_android_back() {
    let released = CLAIM.with(|claim| {
        let mut value = claim.get();
        let released = value.release();
        claim.set(value);
        released
    });
    if released == BackRelease::Stop {
        clear_register_failures();
        stop_listener();
    }
}

fn ensure_listener() {
    if PHASE.with(|phase| phase.get()) != BackPhase::Idle {
        // `Stopping` re-registers from its completion when an overlay is still open.
        return;
    }
    begin_register();
}

fn begin_register() {
    if matches!(
        PHASE.with(|phase| phase.get()),
        BackPhase::Starting | BackPhase::Live
    ) {
        return;
    }
    let generation = GEN.with(|generation| {
        let next = generation.get().wrapping_add(1);
        generation.set(next);
        next
    });
    PHASE.with(|phase| phase.set(BackPhase::Starting));
    spawn_local(async move {
        let handler = Closure::wrap(Box::new(on_android_back) as Box<dyn FnMut(JsValue)>);
        let registered = on_back_button_press(handler.as_ref().unchecked_ref()).await;
        on_registered(generation, registered, handler);
    });
}

fn on_registered(
    generation: u32,
    registered: Result<JsValue, JsValue>,
    handler: Closure<dyn FnMut(JsValue)>,
) {
    if GEN.with(|slot| slot.get()) != generation {
        if let Ok(listener) = registered {
            spawn_local(async move {
                unregister_listener(&listener).await;
                drop(handler);
            });
        }
        return;
    }
    let listener = match registered {
        Ok(listener) => listener,
        Err(err) => {
            PHASE.with(|phase| phase.set(BackPhase::Idle));
            warn(&format!("back-button: could not listen: {err:?}"));
            if claim_count() == 0 {
                clear_register_failures();
                return;
            }
            let failures = record_register_failure();
            if should_retry_register(claim_count(), failures) {
                begin_register();
            }
            return;
        }
    };
    if claim_count() == 0 {
        clear_register_failures();
        PHASE.with(|phase| phase.set(BackPhase::Stopping));
        spawn_local(async move {
            unregister_listener(&listener).await;
            drop(handler);
            finish_unregister(generation);
        });
        return;
    }
    clear_register_failures();
    LISTENER.with(|slot| {
        *slot.borrow_mut() = Some(LiveListener { listener, handler });
    });
    PHASE.with(|phase| phase.set(BackPhase::Live));
}

fn stop_listener() {
    if PHASE.with(|phase| phase.get()) != BackPhase::Live {
        return;
    }
    let generation = GEN.with(|generation| generation.get());
    let live = LISTENER.with(|slot| slot.borrow_mut().take());
    PHASE.with(|phase| phase.set(BackPhase::Stopping));
    spawn_local(async move {
        if let Some(LiveListener { listener, handler }) = live {
            unregister_listener(&listener).await;
            drop(handler);
        }
        finish_unregister(generation);
    });
}

fn finish_unregister(generation: u32) {
    if GEN.with(|slot| slot.get()) != generation {
        return;
    }
    if claim_count() > 0 {
        begin_register();
    } else {
        PHASE.with(|phase| phase.set(BackPhase::Idle));
    }
}

async fn unregister_listener(listener: &JsValue) {
    let Ok(unregister) = js_sys::Reflect::get(listener, &JsValue::from_str("unregister")) else {
        return;
    };
    let Ok(unregister) = unregister.dyn_into::<js_sys::Function>() else {
        return;
    };
    if let Ok(result) = unregister.call0(listener)
        && result.is_instance_of::<js_sys::Promise>()
    {
        let _: Result<JsValue, JsValue> =
            JsFuture::from(js_sys::Promise::unchecked_from_js(result)).await;
    }
}

fn on_android_back(raw: JsValue) {
    let can_go_back = serde_wasm_bindgen::from_value::<BackPayload>(raw)
        .map(|payload| payload.can_go_back)
        .unwrap_or(false);
    match back_action(overlay_presence(), can_go_back) {
        BackAction::Escape => dispatch_escape(),
        BackAction::HistoryBack => history_back(),
        BackAction::Ignore => {}
    }
}

fn overlay_presence() -> OverlayPresence {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return OverlayPresence::Unknown;
    };
    let Some(root) = document.get_element_by_id(OVERLAY_ROOT_ID) else {
        return OverlayPresence::Closed;
    };
    match root.query_selector(&format!("[{OVERLAY_HOST_ATTR}]")) {
        Ok(Some(_)) => OverlayPresence::Open,
        Ok(None) => OverlayPresence::Closed,
        Err(_) => OverlayPresence::Unknown,
    }
}

/// Capture-phase Escape on `document`, the same event [`crate::overlay::use_overlay_keys`] handles.
/// The topmost overlay clears a tag query or closes; a sheet under it ignores it.
pub fn dispatch_escape() {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let init = KeyboardEventInit::new();
    init.set_key("Escape");
    init.set_bubbles(true);
    init.set_cancelable(true);
    let Ok(event) = KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init) else {
        return;
    };
    let _ = document.dispatch_event(&event);
}

fn history_back() {
    let Some(window) = web_sys::window() else {
        return;
    };
    if let Ok(history) = window.history() {
        let _ = history.back();
    }
}

#[cfg(test)]
mod tests;
