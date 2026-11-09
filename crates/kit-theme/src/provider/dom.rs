//! Browser side effects: `data-theme`, the mirror, the OS shade and the
//! Android `SystemBars` bridge. Each one logs and carries on when the API is
//! missing.

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::mirror::MirrorKeys;
use crate::prefs::{Shade, Theme, ThemePrefs};

/// Same query as `js/theme-boot.js`.
const DARK_QUERY: &str = "(prefers-color-scheme: dark)";

fn dark_query() -> Option<web_sys::MediaQueryList> {
    web_sys::window()?.match_media(DARK_QUERY).ok().flatten()
}

pub(super) fn os_shade() -> Shade {
    if dark_query().is_some_and(|query| query.matches()) {
        Shade::Dark
    } else {
        Shade::Light
    }
}

/// Calls `on_change` on every OS light/dark flip. Returns the cleanup that
/// removes the listener.
pub(super) fn watch_os_shade(on_change: impl Fn(Shade) + 'static) -> impl FnOnce() {
    let query = dark_query();
    let listener = Closure::<dyn FnMut()>::new(move || on_change(os_shade()));
    if let Some(query) = &query
        && query
            .add_event_listener_with_callback("change", listener.as_ref().unchecked_ref())
            .is_err()
    {
        warn("theme: could not listen for light/dark changes");
    }
    move || {
        if let Some(query) = query
            && query
                .remove_event_listener_with_callback("change", listener.as_ref().unchecked_ref())
                .is_err()
        {
            warn("theme: could not remove the light/dark listener");
        }
    }
}

pub(super) fn apply(theme: Option<Theme>) {
    let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    else {
        return;
    };
    let applied = match theme {
        Some(theme) => root.set_attribute("data-theme", theme.name()),
        None => root.remove_attribute("data-theme"),
    };
    if applied.is_err() {
        warn("theme: could not set data-theme");
    }
}

pub(super) fn write_mirror(keys: &MirrorKeys, prefs: ThemePrefs) {
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return;
    };
    for (key, value) in keys.entries(prefs) {
        if storage.set_item(key, value).is_err() {
            warn("theme: could not write the localStorage mirror");
            return;
        }
    }
}

/// Android: light or dark status / navigation bar icons through a
/// `SystemBars.setDark(boolean)` bridge. No-op where `window.SystemBars` is
/// missing (desktop, browser).
pub(super) fn set_system_bars(shade: Shade) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(bridge) = js_sys::Reflect::get(&window, &"SystemBars".into())
        .ok()
        .filter(|value| value.is_object())
    else {
        return;
    };
    if let Some(set_dark) = js_sys::Reflect::get(&bridge, &"setDark".into())
        .ok()
        .and_then(|value| value.dyn_into::<js_sys::Function>().ok())
        && set_dark.call1(&bridge, &shade.is_dark().into()).is_err()
    {
        warn("theme: SystemBars.setDark failed");
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub(super) fn warn(message: &str);
}

/// Host builds have no browser console.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn warn(_message: &str) {}
