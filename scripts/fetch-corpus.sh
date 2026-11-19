#!/usr/bin/env bash
# Fetches every repository in corpus/manifest.json at its pinned commit into <dest>.
# Sources are untrusted data: git hooks are disabled, submodules and LFS are skipped, nothing is run.
# Needs git and jq. Usage: fetch-corpus.sh <dest-dir> [name...]
set -euo pipefail

dest="${1:?usage: fetch-corpus.sh <dest-dir> [name...]}"
shift
manifest="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/corpus/manifest.json"
export GIT_LFS_SKIP_SMUDGE=1 GIT_TERMINAL_PROMPT=0 DEST="$dest"

fetch_one() {
  local name="$1" url="$2" commit="$3" dir="$DEST/$1"
  local git=(git -c core.hooksPath=/dev/null -C "$dir")
  # An interrupted fetch leaves a `.git` with no commit: that is "not cached", not an error.
  if [ -d "$dir/.git" ] && [ "$("${git[@]}" rev-parse HEAD 2>/dev/null || true)" = "$commit" ]; then
    echo "cached $name"
    return 0
  fi
  mkdir -p "$dir"
  "${git[@]}" init -q
  "${git[@]}" fetch -q --depth 1 --no-tags --no-recurse-submodules "$url" "$commit"
  "${git[@]}" checkout -q --detach FETCH_HEAD
  echo "fetched $name"
}
export -f fetch_one

only="$(printf '%s\n' "$@" | jq -R . | jq -s .)"
jq -r --argjson only "$only" \
  '.repositories[] | select(($only | length) == 0 or (.name as $n | $only | index($n))) | "\(.name) \(.url) \(.commit)"' \
  "$manifest" |
  xargs -P 6 -n 3 bash -c 'fetch_one "$0" "$1" "$2" || echo "FAILED $0" >&2'
