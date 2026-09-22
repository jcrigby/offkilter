#!/bin/sh
# Stop hook: refuse to end a turn while the checkout has uncommitted
# changes or commits the remote does not have, so work is never left
# only on this machine. Exit 2 with the reason on stderr blocks the
# stop and hands the reason back to Claude.
input=$(cat)
case "$input" in
  *'"stop_hook_active":true'* | *'"stop_hook_active": true'*) exit 0 ;;
esac
root=$(git rev-parse --show-toplevel 2>/dev/null) || exit 0
cd "$root" || exit 0
if [ -n "$(git status --porcelain)" ]; then
  echo "There are uncommitted changes in the repository. Commit and push them to the remote branch before stopping." >&2
  exit 2
fi
branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)
upstream=$(git rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null)
if [ -z "$upstream" ]; then
  echo "Branch $branch has no upstream. Push it with 'git push -u origin $branch' before stopping." >&2
  exit 2
fi
if [ -n "$(git log "$upstream..HEAD" --oneline 2>/dev/null)" ]; then
  echo "Branch $branch has commits that $upstream does not. Push them before stopping." >&2
  exit 2
fi
exit 0
