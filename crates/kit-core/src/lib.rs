//! Identity values each app injects into the kit, parsed once at the edge.
//!
//! Kit crates never hardcode an app's env-var prefix, log tag or storage key
//! prefix. The app parses them here and passes the typed value in.

mod identity;

pub use identity::{EnvPrefix, IdentityKind, LogTag, ParseIdentityError, StoragePrefix};
