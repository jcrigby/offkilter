#!/usr/bin/env bash
# Everything a fresh checkout needs before the commands in CLAUDE.md
# work, in the order they are needed, each step skipped when it is
# already done, so it is safe to run again and is what a cloud
# environment's setup script, Dockerfile.dev and a new machine all run:
#
#   1. rustfmt and clippy for the pinned toolchain (rust-toolchain.toml
#      installs the compiler and the wasm target on the first cargo call)
#   2. wasm-bindgen-cli at the version crates/ok-wasm/Cargo.toml pins
#   3. the web app's npm packages and Playwright's Chromium
#   4. cargo-nextest, which runs the workspace's test binaries in
#      parallel (CLAUDE.md, "Commands")
#   5. the release ok-mcp binary that .mcp.json points at, so the MCP
#      server connects in the first session
#
# Prints what it is doing and how long each step took. `--no-browser`
# skips Playwright's download (a container that already has Chromium
# sets PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD or PW_CHROMIUM and is skipped
# anyway); `--no-web` skips step 3 altogether, for a machine that only
# runs the kernel and the MCP server.
set -euo pipefail
cd "$(dirname "$0")/.."

WEB=1
BROWSER=1
for arg in "$@"; do
  case "$arg" in
    --no-web) WEB=0 ;;
    --no-browser) BROWSER=0 ;;
    *) echo "usage: scripts/setup.sh [--no-web] [--no-browser]" >&2; exit 2 ;;
  esac
done

START=$(date +%s)
step_start=0
step() {
  step_start=$(date +%s)
  printf '\n==> %s\n' "$1"
}
done_step() {
  printf '    %s in %ds\n' "$1" "$(( $(date +%s) - step_start ))"
}

step "rustfmt and clippy for the pinned toolchain"
if cargo fmt --version >/dev/null 2>&1 && cargo clippy --version >/dev/null 2>&1; then
  done_step "already installed"
else
  rustup component add rustfmt clippy
  done_step "installed"
fi

WANT=$(sed -n 's/^wasm-bindgen = "\([0-9.]*\)"/\1/p' crates/ok-wasm/Cargo.toml | head -1)
step "wasm-bindgen-cli $WANT (the version crates/ok-wasm/Cargo.toml pins)"
HAVE=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)
if [ "$HAVE" = "$WANT" ]; then
  done_step "already installed"
else
  cargo install wasm-bindgen-cli --version "$WANT" --locked
  done_step "installed"
fi

if [ "$WEB" = 1 ]; then
  step "the web app's npm packages (apps/web)"
  if [ -f apps/web/node_modules/.package-lock.json ] \
      && [ ! apps/web/package-lock.json -nt apps/web/node_modules/.package-lock.json ]; then
    done_step "already installed"
  else
    (cd apps/web && npm ci)
    done_step "installed"
  fi

  step "Playwright's Chromium"
  if [ "$BROWSER" = 0 ]; then
    done_step "skipped (--no-browser)"
  elif [ -n "${PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD:-}" ] || [ -x "${PW_CHROMIUM:-/nonexistent}" ]; then
    done_step "skipped: this machine already has one (PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD or PW_CHROMIUM is set)"
  else
    # --with-deps wants root or sudo for the system libraries; without
    # either, install the browser alone and say what was skipped.
    if (cd apps/web && npx playwright install --with-deps chromium); then
      done_step "installed with its system libraries"
    else
      (cd apps/web && npx playwright install chromium)
      done_step "installed (system libraries not installed: no root or sudo; run 'npx playwright install-deps chromium' as root if the browser suite fails to launch)"
    fi
  fi
fi

step "cargo-nextest (runs the test binaries in parallel)"
if cargo nextest --version >/dev/null 2>&1; then
  done_step "already installed"
else
  cargo install cargo-nextest --locked
  done_step "installed"
fi

step "the release ok-mcp binary (.mcp.json points at target/release/ok-mcp)"
cargo build --release -p ok-mcp
done_step "built"

printf '\nsetup done in %ds\n' "$(( $(date +%s) - START ))"
