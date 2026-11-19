#!/usr/bin/env bash
# Writes the npm package of one platform: the binary of that platform and the fields npm needs to
# install it only there. Usage: package-npm.sh <platform> <binary> <version> <out-dir>
# Platforms are Node's: darwin-arm64, linux-x64, linux-arm64, win32-x64.
set -euo pipefail

platform="${1:?platform}"
binary="${2:?binary}"
version="${3:?version}"
out="${4:?out-dir}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

os="${platform%%-*}"
cpu="${platform#*-}"
exe="debuggatha"
[ "$os" = "win32" ] && exe="debuggatha.exe"

dir="$out/debuggatha-$platform"
rm -rf "$dir"
mkdir -p "$dir/bin"
cp "$binary" "$dir/bin/$exe"
chmod +x "$dir/bin/$exe"
cp "$root/LICENSE" "$dir/LICENSE"
printf '# @sxnnyside/debuggatha-%s\n\nThe `debuggatha` binary for %s on %s. Install [`@sxnnyside/debuggatha-cli`](https://www.npmjs.com/package/@sxnnyside/debuggatha-cli); it depends on this package where it applies.\n' \
  "$platform" "$os" "$cpu" >"$dir/README.md"

jq -n --arg name "@sxnnyside/debuggatha-$platform" --arg version "$version" --arg os "$os" --arg cpu "$cpu" \
  '{name: $name, version: $version,
    description: "The debuggatha binary for \($os) \($cpu).",
    license: "MIT", author: "Sxnnyside Project",
    repository: {type: "git", url: "git+https://github.com/sxnnyside-project/debuggatha.git", directory: "npm/cli"},
    homepage: "https://github.com/sxnnyside-project/debuggatha#readme",
    os: [$os], cpu: [$cpu], files: ["bin", "README.md", "LICENSE"],
    publishConfig: {access: "public"}}' >"$dir/package.json"
echo "$dir"
