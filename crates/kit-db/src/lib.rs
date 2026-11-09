//! Host-side Turso helpers shared by both apps (sickness_tracker#97).
//!
//! - [`query`]: compile sea-query statements to SQL + Turso params, run them,
//!   read typed columns, and run a closure in a transaction.
//! - [`certs`]: the Mozilla CA bundle Turso's sync TLS needs on Android.
//! - [`paths`]: `<PREFIX>_DATA_DIR` / `_DB_PATH` / `_CLOCK` overrides from an
//!   [`kit_core::EnvPrefix`], and the local `tursodb` CLI lookup for tests.
//! - [`time`]: [`UtcMillis`], the one stored timestamp form.
//!
//! No schema, no DDL, no URLs: each app owns its tables, migrations and sync
//! endpoint. Errors are one typed [`Error`]; each app maps it into its own.
//!
//! Host only: on `wasm32` this crate is empty.
#![cfg(not(target_arch = "wasm32"))]

pub mod certs;
mod error;
pub mod paths;
pub mod query;
pub mod time;

pub use error::{Error, Result};
pub use time::UtcMillis;
