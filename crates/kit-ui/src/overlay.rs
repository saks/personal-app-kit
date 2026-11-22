//! Overlay stack with no markup: one DOM host per mounted overlay inside a
//! shared root, so the last host is topmost. The topmost overlay alone handles
//! Escape, Tab focus wrap and (on Android) Back. Apps build their own sheets and
//! forms on these hooks.

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{AddEventListenerOptions, Element, HtmlElement, KeyboardEvent, NodeList};
use yew::prelude::*;

/// `id` of the single overlay root under `<body>` that holds every overlay host.
pub const OVERLAY_ROOT_ID: &str = "form-overlay-root";
const FOCUSABLE: &str = "button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled])";

/// The overlay root ([`OVERLAY_ROOT_ID`]), created under `<body>` on first use.
/// `None` outside a browser document.
pub fn overlay_root() -> Option<Element> {
    let document = web_sys::window()?.document()?;
    if let Some(existing) = document.get_element_by_id(OVERLAY_ROOT_ID) {
        return Some(existing);
    }
    let host = document.create_element("div").ok()?;
    host.set_id(OVERLAY_ROOT_ID);
    document.body()?.append_child(host.as_ref()).ok()?;
    Some(host)
}

/// Renders `child` into the overlay root, or in place when there is no root.
pub fn portal_overlay(child: Html) -> Html {
    match overlay_root() {
        Some(host) => create_portal(child, host),
        None => child,
    }
}

/// Attribute on each overlay host inside the overlay root.
pub const OVERLAY_HOST_ATTR: &str = "data-overlay-host";

/// One `div[data-overlay-host]` per mounted overlay inside the overlay root, appended on
/// mount and removed on unmount, so the hosts form a stack: the last one is topmost.
/// On Android the mount claims `back-button` until unmount (see [`crate::back::claim_android_back`]).
#[hook]
pub fn use_overlay_host() -> Option<Element> {
    // Creating and appending the host in the `use_state` initializer is a side effect during
    // render; it runs once per mount here. Do not copy it into code that renders twice.
    let host = use_state(|| {
        let root = overlay_root()?;
        let host = root.owner_document()?.create_element("div").ok()?;
        host.set_attribute(OVERLAY_HOST_ATTR, "").ok()?;
        root.append_child(host.as_ref()).ok()?;
        Some(host)
    });
    {
        let host = (*host).clone();
        use_effect_with((), move |_| {
            // After this effect, `use_overlay_keys` installs the Escape listener
            // (hooks run in call order). Claiming here means a Back press is not
            // delivered until that listener exists.
            let back = host.is_some().then(crate::back::claim_android_back);
            move || {
                drop(back);
                if let Some(host) = host {
                    host.remove();
                }
            }
        });
    }
    (*host).clone()
}

/// Renders `child` into `host` (from [`use_overlay_host`]), or in place when there is none.
pub fn portal_into(host: Option<Element>, child: Html) -> Html {
    match host {
        Some(host) => create_portal(child, host),
        None => child,
    }
}

/// Whether `host` is the last overlay host (no overlay was opened after it). No host
/// (not in a browser) counts as topmost.
pub fn is_topmost(host: Option<&Element>) -> bool {
    let Some(host) = host else {
        return true;
    };
    let Some(root) = host.parent_element() else {
        return true;
    };
    let Ok(hosts) = root.query_selector_all(&format!(":scope > [{OVERLAY_HOST_ATTR}]")) else {
        return true;
    };
    hosts
        .item(hosts.length().saturating_sub(1))
        .is_some_and(|last| last.is_same_node(Some(host.as_ref())))
}

/// Tab / Shift+Tab wrap between the first and last enabled control in `panel`.
pub fn trap_tab(event: &KeyboardEvent, panel: &HtmlElement) {
    if event.key() != "Tab" {
        return;
    }
    let Ok(nodes) = panel.query_selector_all(FOCUSABLE) else {
        return;
    };
    let len = nodes.length();
    if len == 0 {
        return;
    }
    let Some(first) = item_element(&nodes, 0) else {
        return;
    };
    let Some(last) = item_element(&nodes, len - 1) else {
        return;
    };
    let Some(active) = event
        .target()
        .and_then(|target| target.dyn_into::<HtmlElement>().ok())
    else {
        return;
    };
    if event.shift_key() {
        if active.is_same_node(Some(first.as_ref())) {
            event.prevent_default();
            let _ = last.focus();
        }
    } else if active.is_same_node(Some(last.as_ref())) {
        event.prevent_default();
        let _ = first.focus();
    }
}

fn item_element(nodes: &NodeList, index: u32) -> Option<HtmlElement> {
    nodes
        .item(index)
        .and_then(|node| node.dyn_into::<HtmlElement>().ok())
}

/// Close action for [`use_overlay_keys`]. `Click` for a close callback that
/// takes the Close button's `MouseEvent`, `Plain` for one that takes `()`.
#[derive(Clone)]
pub enum OverlayClose {
    /// Emits a synthetic `click` `MouseEvent`.
    Click(Callback<MouseEvent>),
    /// Emits `()`.
    Plain(Callback<()>),
}

impl OverlayClose {
    fn emit(&self) {
        match self {
            Self::Click(on_close) => {
                if let Ok(click) = MouseEvent::new("click") {
                    on_close.emit(click);
                }
            }
            Self::Plain(on_close) => on_close.emit(()),
        }
    }
}

/// While mounted and topmost (see [`use_overlay_host`]), Escape closes the overlay and
/// Tab / Shift+Tab wrap focus inside `panel_ref`, so a sheet over a form handles Escape
/// alone. Android Back dispatches this same Escape (see [`crate::back::dispatch_escape`]). One
/// capture-phase `keydown` listener on `document` for the overlay's lifetime; it reads
/// the latest `on_close` from a ref, so parent renders do not re-subscribe.
#[hook]
pub fn use_overlay_keys(on_close: OverlayClose, panel_ref: NodeRef, host: Option<Element>) {
    let latest = use_mut_ref(|| on_close.clone());
    *latest.borrow_mut() = on_close;
    use_effect_with((), move |_| {
        let document = web_sys::window().and_then(|window| window.document());
        let listener = Closure::wrap(Box::new(move |event: KeyboardEvent| {
            if !is_topmost(host.as_ref()) {
                return;
            }
            if event.key() == "Escape" {
                event.prevent_default();
                let on_close = latest.borrow().clone();
                on_close.emit();
                return;
            }
            if let Some(panel) = panel_ref.cast::<HtmlElement>() {
                trap_tab(&event, &panel);
            }
        }) as Box<dyn FnMut(KeyboardEvent)>);
        if let Some(document) = document.as_ref() {
            let capture = AddEventListenerOptions::new();
            capture.set_capture(true);
            let _ = document.add_event_listener_with_callback_and_add_event_listener_options(
                "keydown",
                listener.as_ref().unchecked_ref(),
                &capture,
            );
        }
        move || {
            if let Some(document) = document {
                let _ = document.remove_event_listener_with_callback_and_bool(
                    "keydown",
                    listener.as_ref().unchecked_ref(),
                    true,
                );
            }
        }
    });
}
