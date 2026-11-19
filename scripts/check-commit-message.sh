#!/usr/bin/env bash
set -euo pipefail

MSG_FILE="$1"
subject=$(head -n 1 "$MSG_FILE")
PATTERN="^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([a-zA-Z0-9_.-]+\))?!?: .+$|^(Merge|Revert) .+$"

if ! echo "$subject" | grep -Eq "$PATTERN"; then
  echo "Commit message does not follow Conventional Commits: $subject" >&2
  echo "Format: <type>(<scope>): <subject>" >&2
  echo "Allowed types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert" >&2
  exit 1
fi

