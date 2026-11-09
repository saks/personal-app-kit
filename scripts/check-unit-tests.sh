#!/bin/sh
# Fail if unit tests are inline in a production .rs file.
# Layout: `#[cfg(test)] mod tests;` in the parent, body in tests.rs
# (foo.rs → foo/tests.rs, foo/mod.rs or lib.rs → tests.rs beside it).
# Port of octo-budget-rs scripts/check-unit-tests.sh. Run from `make check`.
set -eu

root="$(CDPATH="" cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$root"

bad="$(
  git ls-files -z --cached --others --exclude-standard '*.rs' | python3 -c '
import os, re, sys

inline_mod = re.compile(r"^\s*mod tests?\s*\{")
test_attr = re.compile(r"^\s*#\[(?:tokio::)?test\b")

hits = []
buf = b""
while True:
    chunk = os.read(0, 65536)
    if not chunk:
        break
    buf += chunk
paths = [p.decode() for p in buf.split(b"\0") if p]
for path in paths:
    with open(path, encoding="utf-8") as fh:
        for i, line in enumerate(fh, 1):
            if inline_mod.search(line):
                hits.append(f"{path}:{i}: inline test module (use tests.rs)")
            if os.path.basename(path) != "tests.rs" and test_attr.search(line):
                hits.append(f"{path}:{i}: #[test] must live in tests.rs")
if hits:
    sys.stdout.write("\n".join(hits) + "\n")
'
)"

if [ -n "$bad" ]; then
  echo "scripts/check-unit-tests.sh: unit tests must be a sibling tests.rs:" >&2
  printf '%s\n' "$bad" | sed 's/^/  /' >&2
  echo "parent file: #[cfg(test)] mod tests;   body: <file>/tests.rs" >&2
  exit 1
fi
