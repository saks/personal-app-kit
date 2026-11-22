//! On-screen keyboard (IME) handling for record forms.
//!
//! [`bind`] keeps the CSS variable `--ime-bottom` on `<html>` equal to the
//! height the keyboard covers and scrolls the focused field into view. While a
//! `.record-field` is focused, a tap on a `.record-form button` keeps that
//! focus (so the keyboard stays up) and still clicks the button. Those class
//! names and the variable are the contract with the app's markup and CSS.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{
    AddEventListenerOptions, Event, HtmlElement, PointerEvent, ScrollBehavior,
    ScrollIntoViewOptions, ScrollLogicalPosition,
};

const MIN_IME_PX: f64 = 80.0;
const FALLBACK_IME_RATIO: f64 = 0.45;
const SUPPRESS_CLICK_MS: i32 = 500;

/// Listeners from [`bind`]. Keep it for the app's lifetime.
pub struct ImeGuard {
    _on_change: Closure<dyn FnMut()>,
    _on_pointer_down: Closure<dyn FnMut(PointerEvent)>,
    _on_pointer_up: Closure<dyn FnMut(PointerEvent)>,
    _on_pointer_cancel: Closure<dyn FnMut(PointerEvent)>,
    _on_click: Closure<dyn FnMut(Event)>,
}

struct PendingTouchClick {
    pointer_id: i32,
    button: HtmlElement,
}

/// Starts IME tracking. `None` outside a browser window.
pub fn bind() -> Option<ImeGuard> {
    let window = web_sys::window()?;
    let on_change = Closure::wrap(Box::new(|| {
        apply_ime_inset();
        reveal_active();
    }) as Box<dyn FnMut()>);
    let cb = on_change.as_ref().unchecked_ref();
    let _ = window.add_event_listener_with_callback("resize", cb);
    let pending = Rc::new(RefCell::new(None::<PendingTouchClick>));
    let suppress_trusted_click = Rc::new(Cell::new(false));
    let on_pointer_down = Closure::wrap({
        let pending = pending.clone();
        Box::new(move |event: PointerEvent| {
            retain_on_pointer_down(&event, &pending);
        }) as Box<dyn FnMut(PointerEvent)>
    });
    let on_pointer_up = Closure::wrap({
        let pending = pending.clone();
        let suppress_trusted_click = suppress_trusted_click.clone();
        Box::new(move |event: PointerEvent| {
            fire_touch_click(&event, &pending, &suppress_trusted_click);
        }) as Box<dyn FnMut(PointerEvent)>
    });
    let on_pointer_cancel = Closure::wrap({
        let pending = pending.clone();
        Box::new(move |event: PointerEvent| {
            clear_pending(&pending, event.pointer_id());
        }) as Box<dyn FnMut(PointerEvent)>
    });
    let on_click = Closure::wrap({
        let suppress_trusted_click = suppress_trusted_click.clone();
        Box::new(move |event: Event| {
            if event.is_trusted() && suppress_trusted_click.get() {
                event.prevent_default();
                event.stop_immediate_propagation();
                suppress_trusted_click.set(false);
            }
        }) as Box<dyn FnMut(Event)>
    });
    if let Some(doc) = window.document() {
        let _ = doc.add_event_listener_with_callback("focusin", cb);
        let _ = doc.add_event_listener_with_callback("focusout", cb);
        let capture = AddEventListenerOptions::new();
        capture.set_capture(true);
        let _ = doc.add_event_listener_with_callback_and_add_event_listener_options(
            "pointerdown",
            on_pointer_down.as_ref().unchecked_ref(),
            &capture,
        );
        let _ = doc.add_event_listener_with_callback_and_add_event_listener_options(
            "pointerup",
            on_pointer_up.as_ref().unchecked_ref(),
            &capture,
        );
        let _ = doc.add_event_listener_with_callback_and_add_event_listener_options(
            "pointercancel",
            on_pointer_cancel.as_ref().unchecked_ref(),
            &capture,
        );
        let _ = doc.add_event_listener_with_callback_and_add_event_listener_options(
            "click",
            on_click.as_ref().unchecked_ref(),
            &capture,
        );
    }
    if let Some(viewport) = window.visual_viewport() {
        let _ = viewport.add_event_listener_with_callback("resize", cb);
        let _ = viewport.add_event_listener_with_callback("scroll", cb);
    }
    apply_ime_inset();
    Some(ImeGuard {
        _on_change: on_change,
        _on_pointer_down: on_pointer_down,
        _on_pointer_up: on_pointer_up,
        _on_pointer_cancel: on_pointer_cancel,
        _on_click: on_click,
    })
}

/// Re-measures the keyboard and scrolls `el` (or its `<label>`) into view.
pub fn reveal_element(el: &HtmlElement) {
    apply_ime_inset();
    scroll_now(el);
    schedule_remeasure();
}

fn retain_on_pointer_down(event: &PointerEvent, pending: &Rc<RefCell<Option<PendingTouchClick>>>) {
    *pending.borrow_mut() = None;
    if !should_retain_field_focus(event) {
        return;
    }
    let Some(button) = overlay_button(event.as_ref()) else {
        return;
    };
    event.prevent_default();
    if event.pointer_type() == "mouse" {
        return;
    }
    *pending.borrow_mut() = Some(PendingTouchClick {
        pointer_id: event.pointer_id(),
        button,
    });
}

fn fire_touch_click(
    event: &PointerEvent,
    pending: &Rc<RefCell<Option<PendingTouchClick>>>,
    suppress_trusted_click: &Rc<Cell<bool>>,
) {
    let Some(pending_click) = pending.borrow_mut().take() else {
        return;
    };
    if pending_click.pointer_id != event.pointer_id() {
        return;
    }
    let Some(up_button) = overlay_button(event.as_ref()) else {
        return;
    };
    if !pending_click.button.is_same_node(Some(up_button.as_ref())) {
        return;
    }
    suppress_trusted_click.set(true);
    up_button.click();
    if let Some(window) = web_sys::window() {
        let suppress_trusted_click = suppress_trusted_click.clone();
        let later = Closure::once_into_js(move || {
            suppress_trusted_click.set(false);
        });
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            later.unchecked_ref(),
            SUPPRESS_CLICK_MS,
        );
    }
}

fn clear_pending(pending: &Rc<RefCell<Option<PendingTouchClick>>>, pointer_id: i32) {
    let mut slot = pending.borrow_mut();
    if slot
        .as_ref()
        .is_some_and(|click| click.pointer_id == pointer_id)
    {
        *slot = None;
    }
}

fn should_retain_field_focus(event: &PointerEvent) -> bool {
    event.button() == 0 && has_on_screen_keyboard() && record_field_focused()
}

fn overlay_button(event: &Event) -> Option<HtmlElement> {
    let target = event.target()?;
    let el = target.dyn_ref::<web_sys::Element>().cloned().or_else(|| {
        target
            .dyn_ref::<web_sys::Node>()
            .and_then(|node| node.parent_element())
    })?;
    let button = el.closest(".record-form button").ok()??;
    if button.has_attribute("disabled") {
        return None;
    }
    button.dyn_into().ok()
}

fn schedule_remeasure() {
    let Some(window) = web_sys::window() else {
        return;
    };
    for delay in [150, 350] {
        let later = Closure::once_into_js(|| {
            apply_ime_inset();
            reveal_active();
        });
        let _ = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(later.unchecked_ref(), delay);
    }
}

fn apply_ime_inset() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let layout_h = window
        .inner_height()
        .ok()
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    let measured = window
        .visual_viewport()
        .map(|viewport| (layout_h - viewport.height() - viewport.offset_top()).max(0.0))
        .unwrap_or(0.0)
        .max(native_ime_px());
    let covered = ime_cover_px(
        has_on_screen_keyboard(),
        record_field_focused(),
        measured,
        layout_h,
    );
    let Some(root) = window.document().and_then(|doc| doc.document_element()) else {
        return;
    };
    let root: HtmlElement = root.unchecked_into();
    let _ = root
        .style()
        .set_property("--ime-bottom", &format!("{covered:.0}px"));
}

fn ime_cover_px(coarse: bool, field_focused: bool, measured: f64, layout_h: f64) -> f64 {
    if !coarse {
        0.0
    } else if field_focused && measured < MIN_IME_PX {
        (layout_h * FALLBACK_IME_RATIO).max(240.0)
    } else {
        measured.max(0.0)
    }
}

fn has_on_screen_keyboard() -> bool {
    web_sys::window()
        .and_then(|window| window.match_media("(pointer: coarse)").ok().flatten())
        .is_some_and(|query| query.matches())
}

fn native_ime_px() -> f64 {
    let Some(window) = web_sys::window() else {
        return 0.0;
    };
    let Ok(insets) = js_sys::Reflect::get(&window, &"SafeAreaInsets".into()) else {
        return 0.0;
    };
    if insets.is_undefined() || insets.is_null() {
        return 0.0;
    }
    let Ok(ime) = js_sys::Reflect::get(&insets, &"ime".into()) else {
        return 0.0;
    };
    let Ok(func) = ime.dyn_into::<js_sys::Function>() else {
        return 0.0;
    };
    func.call0(&insets)
        .ok()
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0)
}

fn record_field_focused() -> bool {
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|doc| doc.active_element())
        .and_then(|el| el.dyn_into::<HtmlElement>().ok())
        .is_some_and(|el| {
            el.class_name()
                .split_whitespace()
                .any(|class| class == "record-field")
        })
}

fn scroll_now(el: &HtmlElement) {
    let target = el
        .closest("label")
        .ok()
        .flatten()
        .and_then(|node| node.dyn_into::<HtmlElement>().ok())
        .unwrap_or_else(|| el.clone());
    let opts = ScrollIntoViewOptions::new();
    opts.set_block(ScrollLogicalPosition::Nearest);
    opts.set_behavior(ScrollBehavior::Instant);
    target.scroll_into_view_with_scroll_into_view_options(&opts);
}

fn reveal_active() {
    let Some(el) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|doc| doc.active_element())
    else {
        return;
    };
    if !el.dyn_ref::<HtmlElement>().is_some_and(|el| {
        el.class_name()
            .split_whitespace()
            .any(|class| class == "record-field")
    }) {
        return;
    }
    scroll_now(&el.unchecked_into());
}

#[cfg(test)]
mod tests;
