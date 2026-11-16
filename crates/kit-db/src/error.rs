use std::path::PathBuf;

/// Everything kit-db can fail with. Each app maps it into its own IPC error.
///
/// `#[non_exhaustive]`: a new variant is not a breaking change, so an app's
/// `match` needs a catch-all arm (map it to a generic error).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Turso(#[from] turso::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    NotUtf8(#[from] std::string::FromUtf8Error),
    #[error("value is not a number")]
    NotANumber,
    #[error("value is not an integer")]
    NotAnInteger,
    #[error("u64 value does not fit in SQLite integer")]
    IntegerOverflow,
    #[error("unsupported sea-query enum value")]
    UnsupportedEnum,
    /// A value from a sea-query type feature (`with-chrono`, `with-json`, …)
    /// that kit-db does not bind; store it as text or an integer instead.
    #[error("unsupported sea-query value type")]
    UnsupportedValue,
    #[error("{var} must be YYYY-MM-DD")]
    InvalidClock { var: String },
    #[error("timestamp is required")]
    EmptyTimestamp,
    #[error("timestamp must be an ISO-8601 datetime")]
    InvalidTimestamp,
    #[error(
        "{test}: tursodb not found (looked in {}); run ./scripts/fetch-tursodb.sh (make check / make e2e)",
        workspace.display()
    )]
    TursodbNotFound { test: String, workspace: PathBuf },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Which call to run again. How long to wait, and how many times, stay the
/// app's policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retry {
    /// Run the same statement again. An open transaction is still open.
    Statement,
    /// Roll the transaction back and run the whole transaction again.
    /// Retrying one statement on a stale snapshot fails the same way.
    Transaction,
}

impl Error {
    /// `Some` when retrying at that level can succeed.
    ///
    /// `Busy` (a lock, or sync apply / checkpoint) is [`Retry::Statement`].
    /// `BusySnapshot` is [`Retry::Transaction`]: turso 0.8.3 leaves the
    /// snapshot stale until the transaction rolls back. A `Busy` message
    /// containing `SQL statements in progress` is `None`. That is
    /// same-connection misuse, and waiting does not finish the other
    /// statement.
    ///
    /// Inside `with_transaction` / `in_transaction`, match this.
    /// [`Self::is_busy`] is true for both levels, so retrying one statement
    /// on [`Retry::Transaction`] fails again.
    pub fn retry(&self) -> Option<Retry> {
        match self {
            Error::Turso(turso::Error::Busy(msg)) if statements_in_progress(msg) => None,
            Error::Turso(turso::Error::Busy(_)) => Some(Retry::Statement),
            Error::Turso(turso::Error::BusySnapshot(_)) => Some(Retry::Transaction),
            Error::Turso(other) if is_busy_message(&other.to_string()) => Some(Retry::Statement),
            _ => None,
        }
    }

    /// [`retry`](Self::retry) is `Some`. Both [`Retry::Statement`] and
    /// [`Retry::Transaction`] count.
    pub fn is_busy(&self) -> bool {
        self.retry().is_some()
    }
}

/// turso 0.8.3 maps `LimboError::StatementsInProgress` onto `Error::Busy`
/// with the suffix `{op} - SQL statements in progress`.
fn statements_in_progress(msg: &str) -> bool {
    msg.to_lowercase().contains("sql statements in progress")
}

/// Text busy from paths that do not return typed `turso::Error::Busy`.
///
/// turso 0.8.3 `sync.rs` `is_sync_busy_error` matches `Error::Error` whose
/// message contains `Database is busy`. `turso_sync_engine` `database_tape.rs`
/// returns `database is busy` for `StepResult::Busy`. The SyncBusy gate
/// returns typed `Busy("database is locked")` and does not use this.
pub(crate) fn is_busy_message(msg: &str) -> bool {
    msg.to_lowercase().contains("database is busy")
}

#[cfg(test)]
mod tests;
