#!/usr/bin/env bash
# The pre-push chain from CLAUDE.md as one command: what the three CI
# jobs (Kernel, Web, End-to-end) run, in their order, with the time each
# step took printed as a table at the end, so a session knows what it
# ran and how long the rest will take.
#
#   scripts/check.sh            the whole chain
#   scripts/check.sh --quick    fmt, clippy and `cargo test -p ok-render`
#                               only: enough for a change confined to
#                               examples/ and crates/ok-render/tests/,
#                               which cannot affect the web app (see
#                               CLAUDE.md, "How work lands")
#   scripts/check.sh --no-browser
#                               everything but the browser suite
#
# Steps run on even after one fails, so one run reports everything;
# the exit status is the number of failed steps. The browser suite wants
# a document server on port 8080: the script starts the debug server it
# just built and stops it after, unless one is already listening.
set -uo pipefail
cd "$(dirname "$0")/.."

QUICK=0
BROWSER=1
for arg in "$@"; do
  case "$arg" in
    --quick) QUICK=1 ;;
    --no-browser) BROWSER=0 ;;
    *) echo "usage: scripts/check.sh [--quick | --no-browser]" >&2; exit 2 ;;
  esac
done

NAMES=()
TIMES=()
RESULTS=()
FAILED=0
START=$(date +%s)
LOG=$(mktemp -t ok-check.XXXXXX.log)

run() {
  # run NAME COMMAND...: prints the step, runs it, records the result.
  local name=$1
  shift
  printf '\n==> %s\n    %s\n' "$name" "$*"
  local t0
  t0=$(date +%s)
  if "$@"; then
    RESULTS+=(ok)
  else
    RESULTS+=(FAILED)
    FAILED=$((FAILED + 1))
  fi
  NAMES+=("$name")
  TIMES+=($(( $(date +%s) - t0 )))
}

fmt_in_place() {
  # cargo fmt formats in place; a change it made is a failure, since the
  # push has to carry the formatted files, so say which ones.
  cargo fmt --all || return 1
  local changed
  changed=$(git diff --name-only -- '*.rs')
  if [ -n "$changed" ]; then
    printf 'cargo fmt changed:\n%s\ncommit them\n' "$changed"
    return 1
  fi
}

e2e() {
  (cd apps/web && npx playwright test)
}

run "fmt" fmt_in_place
run "clippy" cargo clippy --workspace --all-targets -- -D warnings
if [ "$QUICK" = 1 ]; then
  run "test ok-render" cargo test -p ok-render
else
  run "test workspace" cargo test --workspace
  run "wasm" ./scripts/build-wasm.sh
  run "typecheck" npm --prefix apps/web run typecheck
  run "web build" npm --prefix apps/web run build
  if [ "$BROWSER" = 1 ]; then
    run "build ok-server" cargo build -p ok-server
    SERVER_PID=
    if ! curl -fs -o /dev/null http://localhost:8080/ 2>/dev/null; then
      DATA=$(mktemp -d -t ok-check-data.XXXXXX)
      ./target/debug/ok-server --static apps/web/dist --data "$DATA" --port 8080 >"$LOG" 2>&1 &
      SERVER_PID=$!
      for _ in $(seq 1 50); do
        curl -fs -o /dev/null http://localhost:8080/ 2>/dev/null && break
        sleep 0.2
      done
    fi
    # Playwright's own download, or the browser PW_CHROMIUM names (the
    # cloud container and Dockerfile.dev set it).
    run "browser suite" e2e
    if [ -n "$SERVER_PID" ]; then
      kill "$SERVER_PID" 2>/dev/null
      wait "$SERVER_PID" 2>/dev/null
      rm -rf "$DATA"
    fi
  fi
fi

TOTAL=$(( $(date +%s) - START ))
printf '\n%-16s %8s  %s\n' step seconds result
for i in "${!NAMES[@]}"; do
  printf '%-16s %8d  %s\n' "${NAMES[$i]}" "${TIMES[$i]}" "${RESULTS[$i]}"
done
printf '%-16s %8d  %s\n' total "$TOTAL" "$([ "$FAILED" = 0 ] && echo "all ok" || echo "$FAILED failed")"
rm -f "$LOG"
exit "$FAILED"
