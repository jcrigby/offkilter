#!/usr/bin/env bash
# Prints regeneration timings for representative parts (release build).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test --release -p ok-model --test bench -- --ignored --nocapture 2>&1 | grep -E "ms$|ms\)|faces\)|error|panicked"
