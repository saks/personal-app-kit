# Changelog

All kit crates share one version (see `RELEASING.md`). Newest first.

This file starts with the first release. Earlier work is in the git history and the merged PRs.

## Unreleased

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
