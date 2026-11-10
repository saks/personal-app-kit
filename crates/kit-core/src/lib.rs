//! Identity values each app injects into the kit, parsed once at the edge.
//!
//! Kit crates never hardcode an app's env-var prefix, log tag or storage key
//! prefix. The app parses them here and passes the typed value in.

mod identity;

pub use identity::{EnvPrefix, IdentityKind, Literal, LogTag, ParseIdentityError, StoragePrefix};

/// A [`LogTag`] from a string literal, checked at compile time: an invalid
/// tag is a build error, so the value cannot fail at run time.
///
/// ```
/// const TAG: kit_core::LogTag = kit_core::log_tag!("sickness-tracker");
/// assert_eq!(TAG.as_str(), "sickness-tracker");
/// ```
///
/// ```compile_fail
/// let _ = kit_core::log_tag!("not a tag");
/// ```
#[macro_export]
macro_rules! log_tag {
    ($tag:literal) => {
        const {
            match $crate::LogTag::literal($tag) {
                ::core::result::Result::Ok(tag) => $crate::LogTag::from_literal(tag),
                ::core::result::Result::Err(_) => {
                    ::core::panic!(::core::concat!("invalid log tag: ", $tag))
                }
            }
        }
    };
}
