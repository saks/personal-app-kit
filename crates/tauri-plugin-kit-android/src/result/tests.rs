use super::*;

#[test]
fn codes_match_sync_worker_kt() {
    assert_eq!(
        [
            WorkerResult::Success,
            WorkerResult::Retry,
            WorkerResult::Failure
        ]
        .map(WorkerResult::code),
        [0, 1, 2],
        "AttemptBudget.kt's SUCCESS / RETRY / FAILURE"
    );
}

#[test]
fn panic_code_matches_sync_worker_kt() {
    assert_eq!(PANIC_CODE, 3, "AttemptBudget.kt's PANIC");
}
