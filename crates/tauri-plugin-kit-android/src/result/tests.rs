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
        "SyncWorker.kt's SUCCESS / RETRY / FAILURE"
    );
}
