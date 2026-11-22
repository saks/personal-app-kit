//! Logic-only UI hooks shared by the apps' Yew front ends. No markup
//! components: each app keeps its own overlays and sheets and builds them on
//! these.
//!
//! - [`overlay`]: overlay stack (one DOM host per mounted overlay, topmost
//!   handles Escape and Tab), portals.
//! - [`back`]: Android Back as the topmost overlay's Escape while one is open.
//! - [`keyboard`]: on-screen keyboard inset (`--ime-bottom`) and field reveal.
//! - [`pull_refresh`]: pull-to-refresh gesture and its pure offset math.
//!
//! Warnings go through [`set_warn`] (default: `console.warn`), so the app keeps
//! its own log prefix.

use std::cell::Cell;

pub mod back;
pub mod keyboard;
pub mod overlay;
pub mod pull_refresh;

/// Sink for the kit's warnings, for example a failed Android Back register.
pub type WarnFn = fn(&str);

fn console_warn(message: &str) {
    web_sys::console::warn_1(&wasm_bindgen::JsValue::from_str(message));
}

thread_local! {
    static WARN: Cell<WarnFn> = const { Cell::new(console_warn) };
}

/// Routes the kit's warnings to `sink`. Call once at startup; until then they
/// go to `console.warn`.
pub fn set_warn(sink: WarnFn) {
    WARN.with(|slot| slot.set(sink));
}

fn warn(message: &str) {
    let sink = WARN.with(Cell::get);
    sink(message);
}

#[cfg(test)]
mod tests;
