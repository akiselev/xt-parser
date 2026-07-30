#!/usr/bin/env bash
set -euo pipefail

scripts/generate-schemas.sh --check
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
