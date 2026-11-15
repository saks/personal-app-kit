# Changelog

All kit crates share one version (see `RELEASING.md`). Newest first.

## Unreleased

- kit-db: `Error` is now `#[non_exhaustive]`. Apps matching it need a catch-all arm; new variants are no longer breaking.
- kit-db: `Error::UnsupportedValue` for sea-query value types kit-db does not bind (#7).
- kit-db: `write_mozilla_ca_file` writes a per-call temp file, `sync_all`s it and renames it over `cacert.pem`.
- kit-sync-types: `AuthToken` is wiped on drop (`zeroize`).
- tauri-plugin-kit-android: `BackgroundSync::init_logs`, an optional `fn()` the worker calls first in a cold process. Building `BackgroundSync` with a struct literal now needs `init_logs` (`None` keeps today's behaviour).
- tauri-plugin-kit-android: `SyncWorker` fails (no retry) when the native library cannot be loaded.
- kit-core: `const_str::char_at`, the `const` UTF-8 decoder shared with tauri-plugin-kit-android.
