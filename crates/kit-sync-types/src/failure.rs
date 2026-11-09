//! How a failure behaves, whatever app-specific kind it is.

/// Whether a failed Sync can clear without the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FailureClass {
    /// Token rejected, wrong key, bad URL…: retrying won't help; the user
    /// has to act. Shown at once.
    NeedsUser,
    /// Offline, server error, local I/O: a retry may clear it.
    ClearsByItself,
}

/// An app's own failure-kind enum. Implement it with an exhaustive match (no
/// catch-all), so a new kind has to pick its class.
pub trait FailureKind {
    fn class(self) -> FailureClass;
}

#[cfg(test)]
mod tests;
