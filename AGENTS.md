# AGENTS.md

Shared crates for **octo-budget-rs** (OB) and **sickness_tracker** (ST). Both apps consume this workspace as git dependencies pinned by tag. Plan and decisions: [sickness_tracker#87](https://github.com/saks/sickness_tracker/issues/87). Kit issues are tracked there with the `[app-kit]` prefix.

This file is loaded in full on every turn. Keep each rule specific and in one place.

## Layout

| Thing | Where |
|---|---|
| A crate | `crates/<name>/`. Every directory there is a member; do not edit `members` |
| Shared version, edition, lints, dependency versions | root `Cargo.toml` (`[workspace.package]`, `[workspace.lints]`, `[workspace.dependencies]`) |
| Unit tests | `foo/tests.rs` next to `foo.rs`, or `tests.rs` next to `lib.rs` / `mod.rs`. The parent declares `#[cfg(test)] mod tests;`. `scripts/check-unit-tests.sh` enforces it |
| Release procedure | `RELEASING.md` |
| How apps consume the kit | `README.md` |

Each new crate sets `version.workspace = true` (and the other `*.workspace` keys) plus `[lints] workspace = true`. Add a row to the crate table in `README.md` in the same change.

## Design rules

- **Nothing app-specific in a crate.** No URLs, hosts, env-var names, log tags, storage keys, command names or timings. The app passes typed config in (`kit-core` identity newtypes, policy structs).
- Parse, don't validate: newtypes with `FromStr` at the edge, then trust the type. Prefer enums over `bool` / `Option` combinations, and typestate where a state machine has illegal transitions.
- Small files and small modules. Split before a file grows large.
- Wasm-safe crates (used by the Yew UI) must build for `wasm32-unknown-unknown`; `make check` runs clippy for that target.
- daisyUI / Tailwind class names in kit code are full string literals (an enum → `&'static str`), never built with `format!`.
- Wire formats and shared schema files are part of the public API. Changing them is a breaking release.

## Errors

- No `unwrap`, `expect`, `panic!` outside tests (workspace clippy lints, `clippy.toml` allows them in tests).
- Typed error enums with `Display` + `std::error::Error`; each app maps them to its own IPC error.
- No `allow(dead_code)` or `expect(dead_code)`: the workspace forbids `dead_code` and `scripts/check-no-dead-code-allow.sh` greps for it. Delete unused code instead.

## Build / verify

```bash
make check   # layout checks, fmt --check, clippy -D warnings (host + wasm32), cargo test
```

`make check` is the gate; format before claiming done. Clippy (host + wasm32) and tests run with `--all-features`, so code behind a feature such as `yew` is gated too. GitHub Actions is off on purpose; do not add workflows. Every test run is time-bounded (`TEST_TIMEOUT_S`). A change that alters a public API also needs the matching adoption issue or PR in each app.

## Git

- `main` is protected by process: every change is a PR. PRs are squash-merged.
- Never force push. Never rebase pushed work; catch up by merging `main`.
- Tags follow `RELEASING.md`. A breaking change means a new tag; never move a tag.

## Never

- No monkey-patching, vendoring or forking of dependencies, and no committed `[patch]`. Raise it with @saks if it seems unavoidable.
- Never touch prod data or the existing Turso / Turso Cloud databases. Tests use temp files and a local `tursodb` only.
- No Turso access (CLI, API, dashboard) without @saks's direct ask, and double-check even then.
- Do not log, print or commit tokens, sync keys or `.env` files.
- Do not delete anything (branches, tags, issues, files outside your change) without @saks's yes.
