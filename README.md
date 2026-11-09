# personal-app-kit

Shared Rust crates for [octo-budget-rs](https://github.com/saks/octo-budget-rs) and [sickness_tracker](https://github.com/saks/sickness_tracker). Both apps are Rust, Tauri 2, Yew, Tailwind + daisyUI 5, Turso sync and Android. This kit holds the infrastructure they share (build helpers, identity config, sync types, theming, and later DB helpers and UI primitives) so a fix lands once instead of being copied. Domain code (records, budgets, doses, medications) stays in each app.

Plan and decisions: [sickness_tracker#87](https://github.com/saks/sickness_tracker/issues/87).

## Crates

| Crate | Target | What it is |
|---|---|---|
| `kit-core` | host + wasm | App identity newtypes, parsed once: `EnvPrefix`, `LogTag`, `StoragePrefix` |
| `kit-build` | build-dependency | `emit_git_info(&EnvPrefix)`: sets `<PREFIX>_GIT_SHA` and `<PREFIX>_BUILT_AT` for `env!` |
| `kit-theme` | host + wasm (`yew` feature) | Shared daisyUI theme set: `ThemePrefs`/`Theme` enums, `ThemeProvider` + `use_theme` (feature `yew`, persistence via `ThemeApi`, mirror keys from `StoragePrefix`), `css/themes.css`, `js/theme-boot.js` |
| `kit-sync-types` | host + wasm | Plain sync types both engines share: `SyncTrigger`, `FailureClass` + `FailureKind`, `SyncSkip`, `SyncBadge` + `badge(BadgeInput, BadgePolicy)`, `SyncUrl` / `AuthToken` (parsed once, token redacted). No engine, no built-in URLs |
| `tauri-plugin-kit-android` | Android (host for tests; empty on wasm) | Shared WorkManager background-sync job: `SyncWorker` Kotlin library (linked into the app's Gradle build by `build.rs`), `background_sync!(fn() -> BackgroundSync)`, `enqueue()`, `WorkerResult`. JNI symbol fixed by the kit's package |

Every directory under `crates/` is a workspace member. Nothing app-specific is baked into a crate: URLs, env-var prefixes, log tags, storage keys and timings are typed config the app passes in.

## Using it from an app

Add the crates as **git dependencies pinned by tag**. Every kit crate shares one version and one tag, so use the same tag for all of them:

```toml
[build-dependencies]
kit-core = { git = "https://github.com/saks/personal-app-kit", tag = "v0.1.0" }
kit-build = { git = "https://github.com/saks/personal-app-kit", tag = "v0.1.0" }
```

The app's `Cargo.lock` records the exact commit, so builds are reproducible. Bump the tag in its own app PR, behind that app's normal gate.

While a kit PR is still open, an app PR may point at `branch = "<kit-branch>"` to try it, and must switch back to a tag before it merges. Never commit a `[patch]` section.

The repo is private, so cargo needs git credentials on every machine that builds the app, including the Android Gradle → cargo build (`net.git-fetch-with-cli = true` plus a token).

## Releases

See [RELEASING.md](RELEASING.md). In short: one lockstep version for all crates, a tag `vX.Y.Z` only after `make check` passes and @saks merges, and **a breaking change always means a new tag**. A pushed tag is never moved or deleted.

## Develop

Rust stable (pinned in `rust-toolchain.toml`, with `wasm32-unknown-unknown`, rustfmt and clippy), `make`, `git` and `python3`.

```bash
make check   # tests-in-tests.rs and no-allow(dead_code) checks, fmt --check,
             # clippy -D warnings on host and wasm32-unknown-unknown, cargo test
```

`make check` is the gate. GitHub Actions is off on purpose.

## Hard rules

- **No monkey-patching, vendoring or forking** of dependencies. If one seems unavoidable, raise it with @saks first and patch only when he explicitly asks.
- **Never touch prod data or the existing Turso / Turso Cloud databases.** Tests use local files and a local `tursodb` only.
- **No Turso access** of any kind (CLI, API, Cloud dashboard) without @saks's direct ask, and even then double-check before acting.
- No hardcoded database URLs, hosts or tokens. Never log or commit a token or sync key.
- No `unwrap`, `expect` or `panic!` outside tests, and no `allow(dead_code)`. The workspace lints and `make check` enforce both.
