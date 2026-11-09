# Releasing

All kit crates share `[workspace.package] version` in the root `Cargo.toml` and release together under one tag. Apps never mix kit versions.

## Rules

- Tags are `vX.Y.Z` and match the workspace version.
- **A breaking change means a new tag.** While the version is `0.y.z`, a breaking change bumps `y` (`v0.1.3` → `v0.2.0`) and anything else bumps `z`. From `1.0.0`, follow semver.
- A tag is cut only after `make check` passes on the merged `main` and @saks has merged the PR. GitHub Actions is off, so the local gate is the only gate.
- A pushed tag is never moved, deleted or reused. A bad release is fixed by a new tag.

## Steps

1. In the release PR, set `version` in the root `Cargo.toml` and run `make check`.
2. After @saks merges, check out the merged `main`, run `make check` again, then:

   ```bash
   git tag -a vX.Y.Z -m "personal-app-kit vX.Y.Z"
   git push origin vX.Y.Z
   ```

3. Each app moves to the new tag in its own PR, behind its own gate.
