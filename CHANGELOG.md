# Changelog

All kit crates share one version (see `RELEASING.md`). Newest first.

This file starts with the first release. Earlier work is in the git history and the merged PRs.

## Unreleased

- tauri-plugin-kit-android: a background sync stops when the failure cannot clear itself. An unreadable data dir fails the job. A caught panic is retried once, then the job fails. A process that dies in the native call (release builds abort on panic) stops after five deaths, before the library is loaded again. An unknown result code is logged and fails the job. Offline and busy results still retry with WorkManager's backoff. The next save or pause still replaces the job and starts a fresh budget.

- tauri-plugin-kit-android: `Schedule`'s fields are private. Build one with `schedule!` (checked at compile time) or `Schedule::new`. Both delays are a whole number of milliseconds. Backoff must be from 10 s to 5 h, WorkManager's own limits. The initial delay must be at most `Schedule::MAX_INITIAL_DELAY`, which `setInitialDelay` accepts. The bridge sends milliseconds, so 10900 ms stays 10900 ms.

  Migration: replace `Schedule { initial_delay, backoff }` with `schedule!(initial_delay, backoff)`.
- tauri-plugin-kit-android: `enqueue` returns `Result<(), EnqueueError>`. `NoAndroidContext` means there is no application `Context` yet. `Jni` is the JNI or WorkManager failure as text, so the app does not depend on `jni`. `EnqueueError` is `#[non_exhaustive]`. A failure is still logged at `warn`. Success is `debug` (`sync: background job enqueued`).

  Migration: handle the `Result` at each call (`?` or `match`). A `match` needs a catch-all arm. The device-test warm path confirms the job with `dumpsys` or the WorkManager inspector. octo-budget-rs and sickness_tracker do this when they take the next tag.

- tauri-plugin-kit-android: `enqueue` in a cold WorkManager process uses the application `Context` `SyncWorker` passed to `runSync`, so a missing tao `ndk-context` is a logged error instead of a panic. The in-app call, after the Activity has started, is unchanged.

- kit-db: `query` and `Prepared::query` return `Vec<turso::Row>` after the statement has been stepped. Execution errors, including busy, come back from that call (`Error::is_busy`) and are logged with the SQL. Busy is logged at `debug`.

  Migration, when an app switches its reads to the kit:

  1. Replace `let mut rows = query(...).await?; while let Some(row) = rows.next().await?` with `let rows = query(...).await?; for row in rows`.
  2. Replace `rows.next().await?.is_some()` with `!rows.is_empty()`. Exists statements already use `LIMIT 1`.
  3. Retry `query(...).await` itself. A busy read is `Error::is_busy()` from that call.

- kit-db: `query`, `execute`, `Prepared::query`, `Prepared::execute`, `begin`, and `with_transaction` take `&Conn` instead of `&turso::Connection`. `in_transaction` takes `&Conn` and the transaction. `Conn` clones share one connection, so a cancelled or panicking transaction rolls back on the next statement of any clone instead of dropping that clone's write.

  Migration:

  1. Wrap `database.connect()` in `kit_db::query::Conn::new`.
  2. Pass `&Conn` to `query`, `execute`, `Prepared::query`, `Prepared::execute`, `begin`, `with_transaction`, and `in_transaction`.
  3. The closure receives `&Conn`. Use `query` / `execute` for sea-query, or `tx.connection()` for raw SQL.
  4. Share the handle with `Conn::clone`. Do not call `Connection::clone` on `connection()`.
  5. After a cancel or panic, the next statement on any clone rolls the open transaction back and then runs. Do not treat the connection as failed, and do not `COMMIT` to finish a cancelled transaction.

- kit-db: an engine-side rollback (for example `ON CONFLICT ROLLBACK`) no longer leaves the connection failing every later call. `in_transaction` still returns the closure's error.
- kit-sync: `on_suspended` ends the `Auto` debounce window while automatic runs are active, so the next resume syncs after a suspend even when the monotonic clock barely moved.
- kit-sync: `Host::suspended` receives the one pending count from `on_suspended`, then `Flush` runs unless the count is `Ok(0)`. Not called when automatic runs are inactive. octo-budget-rs moves `android_sync::enqueue()` into `Host::suspended`, enqueues when the count isn't `Ok(0)`, and drops its extra `pending()` call. (#18)
- kit-sync: `AutoSync::backoff_attempts` is the backoff counter (renamed from `failures`). It is not a UI failure streak. (#18)
- kit-sync-types: `SyncTrigger::from_host` is dropped. That routing is sickness_tracker-specific and wrong for apps that start `Auto`/`Online` from the host; keep the rule in the app. (#18)
- kit-sync: `Flight` is `#[must_use]`. `admit` starts the flight; `Admission::into_flight` takes the `Flight` out of the result. Dropping a `Flight` still ends the flight and drops a queued run. (#18)
- kit-db: `Error` is now `#[non_exhaustive]`. Apps matching it need a catch-all arm; new variants are no longer breaking. (#15)
- kit-db: `Error::UnsupportedValue` for sea-query value types kit-db does not bind (#7).
- kit-db: `write_mozilla_ca_file` writes a per-call temp file, `sync_all`s it and renames it over `cacert.pem`. (#15)
- kit-sync-types: `AuthToken` is wiped on drop (`zeroize`) (#14).
- tauri-plugin-kit-android: `BackgroundSync::init_logs`, an optional `fn()` the worker calls first in a cold process. Building `BackgroundSync` with a struct literal now needs `init_logs` (`None` keeps today's behaviour). (#15)
- tauri-plugin-kit-android: `SyncWorker` fails (no retry) when the native library cannot be loaded. (#15)
- kit-core: `const_str::char_at`, the `const` UTF-8 decoder shared with tauri-plugin-kit-android. (#15)
