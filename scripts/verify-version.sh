#!/usr/bin/env bash
# Fails unless every published manifest declares the version given (the tag without its `v`).
# The crates, the npm launchers and their platform dependencies, the MCP registry manifest and the
# extension move together. Usage: verify-version.sh <version>
set -euo pipefail

want="${1:?version}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bad=0
expect() {
  if [ "$2" != "$want" ]; then
    echo "version mismatch: $1 is $2, the tag is $want" >&2
    bad=1
  fi
}

for v in $(cargo metadata --no-deps --format-version 1 --manifest-path "$root/Cargo.toml" | jq -r '.packages[] | "\(.name)=\(.version)"'); do
  expect "crate ${v%%=*}" "${v#*=}"
done
expect "npm/cli" "$(jq -r .version "$root/npm/cli/package.json")"
expect "npm/mcp" "$(jq -r .version "$root/npm/mcp/package.json")"
expect "npm/mcp dependency on the cli" "$(jq -r '.dependencies["@sxnnyside/debuggatha-cli"]' "$root/npm/mcp/package.json")"
for dep in $(jq -r '.optionalDependencies | to_entries[] | "\(.key)=\(.value)"' "$root/npm/cli/package.json"); do
  expect "npm/cli optional ${dep%%=*}" "${dep#*=}"
done
expect "server.json" "$(jq -r .version "$root/npm/mcp/server.json")"
expect "server.json package" "$(jq -r '.packages[0].version' "$root/npm/mcp/server.json")"
expect "apps/vscode" "$(jq -r .version "$root/apps/vscode/package.json")"
exit "$bad"
