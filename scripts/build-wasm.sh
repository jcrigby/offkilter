#!/usr/bin/env bash
# Builds the kernel to WebAssembly and generates JS bindings into the web app.
set -euo pipefail
cd "$(dirname "$0")/.."
PROFILE="${1:-release}"
if [ "$PROFILE" = "release" ]; then FLAG=--release; else FLAG=; fi
cargo build -p ok-wasm --target wasm32-unknown-unknown $FLAG
wasm-bindgen --target web --out-dir apps/web/src/wasm --out-name ok_wasm \
  "target/wasm32-unknown-unknown/$PROFILE/ok_wasm.wasm"
# Optimise the release module with wasm-opt when it is available: on PATH
# (binaryen) or from the web app's dev dependencies (the binaryen npm package).
if [ "$PROFILE" = "release" ]; then
  WASM_OPT=""
  if command -v wasm-opt >/dev/null 2>&1; then WASM_OPT=wasm-opt; fi
  if [ -x apps/web/node_modules/.bin/wasm-opt ]; then WASM_OPT=apps/web/node_modules/.bin/wasm-opt; fi
  if [ -n "$WASM_OPT" ]; then
    BEFORE=$(wc -c < apps/web/src/wasm/ok_wasm_bg.wasm)
    "$WASM_OPT" -O3 --enable-bulk-memory --enable-nontrapping-float-to-int \
      apps/web/src/wasm/ok_wasm_bg.wasm -o apps/web/src/wasm/ok_wasm_bg.wasm
    AFTER=$(wc -c < apps/web/src/wasm/ok_wasm_bg.wasm)
    echo "wasm-opt: $BEFORE -> $AFTER bytes"
  else
    echo "wasm-opt not found; skipping optimisation (npm install in apps/web provides it)"
  fi
fi
echo "wasm bindings written to apps/web/src/wasm"
