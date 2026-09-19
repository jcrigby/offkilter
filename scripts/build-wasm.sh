#!/usr/bin/env bash
# Builds the kernel to WebAssembly and generates JS bindings into the web app.
set -euo pipefail
cd "$(dirname "$0")/.."
PROFILE="${1:-release}"
if [ "$PROFILE" = "release" ]; then FLAG=--release; else FLAG=; fi
cargo build -p ok-wasm --target wasm32-unknown-unknown $FLAG
wasm-bindgen --target web --out-dir apps/web/src/wasm --out-name ok_wasm \
  "target/wasm32-unknown-unknown/$PROFILE/ok_wasm.wasm"
echo "wasm bindings written to apps/web/src/wasm"
