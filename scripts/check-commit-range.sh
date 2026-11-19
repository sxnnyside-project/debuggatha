#!/usr/bin/env bash
set -euo pipefail

# Validates commits in a range against Conventional Commits:
# type(scope)!: description
# Types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert

RANGE="${1:-origin/main..HEAD}"
PATTERN="^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([a-zA-Z0-9_.-]+\))?!?: .+$|^(Merge|Revert) .+$"

failed=0
while IFS= read -r sha; do
  [ -z "$sha" ] && continue
  subject=$(git log -1 --format="%s" "$sha")
  if ! echo "$subject" | grep -Eq "$PATTERN"; then
    echo "Commit $sha does not follow Conventional Commits: $subject" >&2
    failed=1
  fi
done < <(git rev-list "$RANGE")

if [ "$failed" -ne 0 ]; then
  echo "Commit validation failed." >&2
  exit 1
fi

