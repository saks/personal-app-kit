use std::time::Duration;

use tokio::time::Instant;

use super::*;

#[tokio::test(start_paused = true)]
async fn a_stalled_future_exceeds_its_budget() {
    let budget = Duration::from_secs(15);
    let started = Instant::now();
    let result = with_budget(budget, std::future::pending::<()>()).await;
    assert_eq!(result, Err(BudgetExceeded { budget }));
    assert_eq!(started.elapsed(), budget);
    assert_eq!(
        BudgetExceeded { budget }.to_string(),
        "timed out after 15000 ms"
    );
}

#[tokio::test(start_paused = true)]
async fn a_future_within_its_budget_returns_its_output() {
    let slow = async {
        tokio::time::sleep(Duration::from_secs(14)).await;
        7
    };
    assert_eq!(with_budget(Duration::from_secs(15), slow).await, Ok(7));
}
