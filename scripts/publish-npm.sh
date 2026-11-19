#!/usr/bin/env bash
# Publishes the npm package in <dir> with provenance, unless that version is already on the
# registry (a release that failed halfway can be run again). Usage: publish-npm.sh <dir>
set -euo pipefail

dir="${1:?package directory}"
name="$(jq -r .name "$dir/package.json")"
version="$(jq -r .version "$dir/package.json")"
if npm view "$name@$version" version >/dev/null 2>&1; then
  echo "$name@$version is already published"
  exit 0
fi
(cd "$dir" && npm publish --provenance --access public)
