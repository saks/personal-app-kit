//! Plain sync types shared by both apps' engines (sickness_tracker#92):
//! ST's sealed engine and OB's plain replica. No engine lives here, and the
//! crate is wasm-safe.
//!
//! - [`SyncTrigger`]: what started a Sync, the union of both apps.
//! - [`FailureClass`] + [`FailureKind`]: each app keeps its own failure-kind
//!   enum and classifies it with an exhaustive match.
//! - [`SyncSkip`]: why a Sync request did nothing.
//! - [`SyncBadge`] + [`badge`]: the dock / Settings mark, with the warning
//!   threshold as [`BadgePolicy`].
//! - [`SyncUrl`] and [`AuthToken`]: the connection, parsed once at the edge.
//!   The user enters both; no URL or host is built in.

mod badge;
mod failure;
mod skip;
mod token;
mod trigger;
mod url;

pub use badge::{Activity, BadgeInput, BadgePolicy, LastRun, OnState, Queue, SyncBadge, badge};
pub use failure::{FailureClass, FailureKind};
pub use skip::SyncSkip;
pub use token::{AuthToken, TokenError};
pub use trigger::SyncTrigger;
pub use url::{SyncUrl, UrlError};
