//! A time budget for one push or pull.

use std::fmt;
use std::future::Future;
use std::time::Duration;

/// A future did not finish within its budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BudgetExceeded {
    /// The budget it ran out of.
    pub budget: Duration,
}

impl fmt::Display for BudgetExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "timed out after {} ms", self.budget.as_millis())
    }
}

impl std::error::Error for BudgetExceeded {}

/// Run `fut` for at most `budget`; past it, drop `fut` and return
/// [`BudgetExceeded`]. Each engine wraps its push and pull with this (see
/// [`crate::SyncPolicy::push_budget`]) and maps the error into its own.
pub async fn with_budget<F: Future>(budget: Duration, fut: F) -> Result<F::Output, BudgetExceeded> {
    tokio::time::timeout(budget, fut)
        .await
        .map_err(|_elapsed| BudgetExceeded { budget })
}

#[cfg(test)]
mod tests;
