#!/bin/sh
# SessionStart hook: builds the release ok-mcp binary that .mcp.json
# points at when it is missing or older than the crates it is built
# from, so the MCP server connects on the first prompt instead of
# opening the session with "offkilter failed to connect". A fresh
# clone that has not run scripts/setup.sh pays for the build here, once;
# every other session start is one stat and a find.
cd "${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}" || exit 0
bin=target/release/ok-mcp
if [ -x "$bin" ]; then
  stale=$(find crates/ok-mcp crates/ok-model Cargo.lock -newer "$bin" -type f 2>/dev/null | head -1)
  [ -z "$stale" ] && exit 0
  echo "ok-mcp is older than $stale; rebuilding it" >&2
else
  echo "no $bin; building it so the MCP server connects (scripts/setup.sh does this and the rest)" >&2
fi
cargo build --release -p ok-mcp >&2
