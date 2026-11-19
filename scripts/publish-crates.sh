#!/usr/bin/env bash
# Publishes the workspace crates in dependency order, skipping a version crates.io already has (a
# release that failed halfway can be run again). Needs CARGO_REGISTRY_TOKEN. Usage: publish-crates.sh
set -euo pipefail

version="$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')"
for crate in debuggatha-core debuggatha-engine debuggatha-cli; do
  if curl -fsS -o /dev/null -A "debuggatha-release" "https://crates.io/api/v1/crates/$crate/$version"; then
    echo "$crate@$version is already published"
    continue
  fi
  cargo publish -p "$crate" --locked
done
