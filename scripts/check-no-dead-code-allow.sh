#!/bin/sh
# Fail on `allow(dead_code)` / `expect(dead_code)` in any .rs file, including
# `cfg_attr(..)` forms. The workspace `dead_code = "forbid"` lint catches the
# compiled ones; this also catches code behind a cfg that is not built here.
set -eu

root="$(CDPATH="" cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$root"

if git ls-files -z --cached --others --exclude-standard '*.rs' \
  | xargs -0 -r grep -nE '(allow|expect)[[:space:]]*\([^)]*dead_code'; then
  echo "scripts/check-no-dead-code-allow.sh: remove the dead code instead of allowing it" >&2
  exit 1
fi
