//! Throwaway crate for the Tailwind-across-crates spike (sickness_tracker#90).
//!
//! Every class below appears only in this crate, as a full string literal, so
//! a consumer's generated CSS contains them only if Tailwind scanned this
//! crate's sources through the resolver's `@source` line. Removed before merge.

use yew::{Html, function_component, html};

/// Badge tone. Each variant maps to one full daisyUI class literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Accent,
    Warning,
}

impl Tone {
    pub const fn class(self) -> &'static str {
        match self {
            Self::Accent => "badge badge-accent badge-xl",
            Self::Warning => "badge badge-warning badge-outline",
        }
    }
}

/// A component whose classes exist nowhere else in the consumer.
#[function_component(SpikeBadge)]
pub fn spike_badge() -> Html {
    html! {
        <div class="kit-spike-marker tracking-[0.37em] skew-y-3 outline-dashed">
            <span class={Tone::Accent.class()}>{ "kit accent" }</span>
            <span class={Tone::Warning.class()}>{ "kit warning" }</span>
        </div>
    }
}

#[cfg(test)]
mod tests;
