//! Pull-to-refresh on a scroll container: damped drag, threshold trigger and a
//! suppressed click after a pull. The app renders the indicator from the offset.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{AddEventListenerOptions, Event, HtmlElement, PointerEvent};
use yew::prelude::*;

const RESISTANCE: f64 = 0.4;
const MAX_PULL: f64 = 120.0;
const PULL_SLOP: f64 = 8.0;

/// Pull offset (px, after resistance) at which release triggers a refresh.
pub const THRESHOLD: f64 = 80.0;
/// Indicator height (px) the app holds while a refresh runs.
pub const REFRESHING_H: f64 = 56.0;

/// Indicator offset for a raw downward drag of `raw_dy` px: zero upward, damped, capped.
pub fn pull_offset(raw_dy: f64) -> f64 {
    if raw_dy <= 0.0 {
        0.0
    } else {
        (raw_dy * RESISTANCE).min(MAX_PULL)
    }
}

/// Whether releasing at `offset` triggers a refresh.
pub fn should_trigger(offset: f64) -> bool {
    offset >= THRESHOLD
}

/// Live pull-to-refresh config, read on every pointer event.
pub struct RefreshCfg {
    /// Called on a release past [`THRESHOLD`]. `None` disables the gesture.
    pub on_refresh: Option<Callback<()>>,
    /// A release may trigger `on_refresh`.
    pub enabled: bool,
    /// A refresh is running: no new pull starts and none triggers.
    pub refreshing: bool,
}

struct Drag {
    pointer_id: Option<i32>,
    start_y: f64,
    offset: f64,
    pulling: bool,
}

impl Drag {
    fn new() -> Self {
        Self {
            pointer_id: None,
            start_y: 0.0,
            offset: 0.0,
            pulling: false,
        }
    }

    fn reset(&mut self) {
        self.pointer_id = None;
        self.start_y = 0.0;
        self.offset = 0.0;
        self.pulling = false;
    }
}

/// Listeners from [`bind`]; dropping it removes them.
pub struct PullGuard {
    el: HtmlElement,
    on_down: Closure<dyn FnMut(PointerEvent)>,
    on_move: Closure<dyn FnMut(PointerEvent)>,
    on_up: Closure<dyn FnMut(PointerEvent)>,
    on_cancel: Closure<dyn FnMut(PointerEvent)>,
    on_touch_move: Closure<dyn FnMut(Event)>,
    on_click: Closure<dyn FnMut(Event)>,
}

/// Binds pull-to-refresh on the scroll container `el`. The drag offset is
/// written to `indicator`; the app renders the indicator.
pub fn bind(
    el: &HtmlElement,
    cfg: Rc<RefCell<RefreshCfg>>,
    indicator: UseStateHandle<f64>,
) -> PullGuard {
    let el = el.clone();
    let drag = Rc::new(RefCell::new(Drag::new()));
    let suppress_click = Rc::new(Cell::new(false));
    let passive_false = AddEventListenerOptions::new();
    passive_false.set_passive(false);
    let capture = AddEventListenerOptions::new();
    capture.set_capture(true);

    let on_down = Closure::wrap({
        let el = el.clone();
        let drag = drag.clone();
        let cfg = cfg.clone();
        Box::new(move |event: PointerEvent| {
            let cfg = cfg.borrow();
            if cfg.refreshing || cfg.on_refresh.is_none() {
                return;
            }
            if el.scroll_top() > 0 {
                return;
            }
            let mut drag = drag.borrow_mut();
            if drag.pointer_id.is_some() {
                return;
            }
            drag.pointer_id = Some(event.pointer_id());
            drag.start_y = f64::from(event.client_y());
            drag.offset = 0.0;
            drag.pulling = false;
        }) as Box<dyn FnMut(PointerEvent)>
    });

    let on_move = Closure::wrap({
        let el = el.clone();
        let drag = drag.clone();
        let indicator = indicator.clone();
        Box::new(move |event: PointerEvent| {
            let mut drag = drag.borrow_mut();
            if drag.pointer_id != Some(event.pointer_id()) {
                return;
            }
            let dy = f64::from(event.client_y()) - drag.start_y;
            let offset = pull_offset(dy);
            if offset > 0.0 && dy > PULL_SLOP {
                event.prevent_default();
                if !drag.pulling {
                    let _ = el.set_pointer_capture(event.pointer_id());
                    drag.pulling = true;
                }
            }
            if drag.pulling {
                event.prevent_default();
                drag.offset = offset;
                indicator.set(offset);
            }
        }) as Box<dyn FnMut(PointerEvent)>
    });

    let finish = {
        let el = el.clone();
        let drag = drag.clone();
        let indicator = indicator.clone();
        let cfg = cfg.clone();
        let suppress_click = suppress_click.clone();
        Rc::new(move |pointer_id: i32, trigger: bool| {
            let mut drag = drag.borrow_mut();
            if drag.pointer_id != Some(pointer_id) {
                return;
            }
            let offset = drag.offset;
            let pulling = drag.pulling;
            drag.reset();
            let _ = el.release_pointer_capture(pointer_id);
            if pulling {
                suppress_click.set(true);
            }
            let cfg = cfg.borrow();
            indicator.set(0.0);
            if trigger
                && pulling
                && should_trigger(offset)
                && cfg.enabled
                && !cfg.refreshing
                && let Some(cb) = cfg.on_refresh.clone()
            {
                cb.emit(());
            }
        })
    };

    let on_up = Closure::wrap({
        let finish = finish.clone();
        Box::new(move |event: PointerEvent| {
            finish(event.pointer_id(), true);
        }) as Box<dyn FnMut(PointerEvent)>
    });
    let on_cancel = Closure::wrap({
        let finish = finish.clone();
        Box::new(move |event: PointerEvent| {
            finish(event.pointer_id(), false);
        }) as Box<dyn FnMut(PointerEvent)>
    });
    let on_touch_move = Closure::wrap({
        let drag = drag.clone();
        Box::new(move |event: Event| {
            if drag.borrow().pulling {
                event.prevent_default();
            }
        }) as Box<dyn FnMut(Event)>
    });
    let on_click = Closure::wrap({
        let suppress_click = suppress_click.clone();
        Box::new(move |event: Event| {
            if suppress_click.get() {
                event.prevent_default();
                event.stop_immediate_propagation();
                suppress_click.set(false);
            }
        }) as Box<dyn FnMut(Event)>
    });

    let _ = el.add_event_listener_with_callback_and_add_event_listener_options(
        "pointerdown",
        on_down.as_ref().unchecked_ref(),
        &passive_false,
    );
    let _ = el.add_event_listener_with_callback_and_add_event_listener_options(
        "pointermove",
        on_move.as_ref().unchecked_ref(),
        &passive_false,
    );
    let _ = el.add_event_listener_with_callback("pointerup", on_up.as_ref().unchecked_ref());
    let _ =
        el.add_event_listener_with_callback("pointercancel", on_cancel.as_ref().unchecked_ref());
    let _ = el.add_event_listener_with_callback_and_add_event_listener_options(
        "touchmove",
        on_touch_move.as_ref().unchecked_ref(),
        &passive_false,
    );
    let _ = el.add_event_listener_with_callback_and_add_event_listener_options(
        "click",
        on_click.as_ref().unchecked_ref(),
        &capture,
    );

    PullGuard {
        el,
        on_down,
        on_move,
        on_up,
        on_cancel,
        on_touch_move,
        on_click,
    }
}

impl Drop for PullGuard {
    fn drop(&mut self) {
        let el = &self.el;
        let _ = el.remove_event_listener_with_callback(
            "pointerdown",
            self.on_down.as_ref().unchecked_ref(),
        );
        let _ = el.remove_event_listener_with_callback(
            "pointermove",
            self.on_move.as_ref().unchecked_ref(),
        );
        let _ = el
            .remove_event_listener_with_callback("pointerup", self.on_up.as_ref().unchecked_ref());
        let _ = el.remove_event_listener_with_callback(
            "pointercancel",
            self.on_cancel.as_ref().unchecked_ref(),
        );
        let _ = el.remove_event_listener_with_callback(
            "touchmove",
            self.on_touch_move.as_ref().unchecked_ref(),
        );
        let _ = el.remove_event_listener_with_callback_and_bool(
            "click",
            self.on_click.as_ref().unchecked_ref(),
            true,
        );
    }
}

#[cfg(test)]
mod tests;
