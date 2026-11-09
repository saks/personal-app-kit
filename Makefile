# personal-app-kit gate. GitHub Actions is off on purpose: `make check` is the gate.
#
#   make check   # layout checks, fmt --check, clippy -D warnings (host + wasm32), tests

WASM_TARGET := wasm32-unknown-unknown

# Upper bound for `cargo test`, so a hung test fails instead of waiting forever.
# GNU `timeout` (Linux) or `gtimeout` (macOS coreutils); with neither, tests run unbounded.
TEST_TIMEOUT_S ?= 600
TIMEOUT_BIN := $(shell command -v timeout 2>/dev/null || command -v gtimeout 2>/dev/null)
BOUNDED := $(if $(TIMEOUT_BIN),$(TIMEOUT_BIN) $(TEST_TIMEOUT_S),)

.PHONY: help check layout fmt clippy clippy-wasm test

help:
	@echo "check         layout + fmt --check + clippy -D warnings (host, $(WASM_TARGET)) + tests"

check: layout fmt clippy clippy-wasm test

# Unit tests live in tests.rs files; no allow(dead_code) anywhere.
layout:
	./scripts/check-unit-tests.sh
	./scripts/check-no-dead-code-allow.sh

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

clippy-wasm:
	cargo clippy --workspace --all-targets --all-features --locked --target $(WASM_TARGET) -- -D warnings

test:
	$(BOUNDED) cargo test --workspace --all-features --locked
