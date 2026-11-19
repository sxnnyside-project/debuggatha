# Command surface for the whole repository.
# The native engine (Cargo workspace under crates/) is the product, and the recipes without a
# prefix drive it. The npm launchers (npm/) and the VS Code extension (apps/vscode, Node + pnpm)
# are clients of the binary, and `vscode-*` recipes drive the extension.

vscode := "apps/vscode"
host_os := if os() == "macos" { "darwin" } else if os() == "windows" { "windows" } else { "linux" }
host_arch := if arch() == "aarch64" { "arm64" } else { "x64" }
exe := if os() == "windows" { ".exe" } else { "" }

default:
    @just --list

# --- Native engine -------------------------------------------------------------------------

# Fetch the crates the lockfile names (the toolchain is pinned by rust-toolchain.toml) and use the repository's git hooks.
install:
    cargo fetch --locked
    git config core.hooksPath .githooks

# The release binary: target/release/debuggatha.
build:
    cargo build --release --locked -p debuggatha-cli

test:
    cargo test --workspace --locked

lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings

# Apply formatting.
format:
    cargo fmt --all

# Verify formatting without writing.
format-check:
    cargo fmt --all --check

# The API documentation, with every warning denied: a missing doc, a broken link, a bad code block.
docs:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items --locked

# Native gate: format, clippy with warnings denied, tests, API docs, dependency policy (needs cargo-deny).
rust-check: format-check lint test docs
    cargo deny check

# Full quality gate. CI runs exactly this: the native gate, the extension's, and the npm packages'.
check: rust-check vscode-check npm-check

# The release binary for this machine in dist/binaries (the other platforms are built by the release workflow on native runners).
binaries: build
    mkdir -p dist/binaries
    cp target/release/debuggatha{{exe}} dist/binaries/debuggatha-{{host_os}}-{{host_arch}}{{exe}}

# Put `debuggatha` on your PATH from this checkout (cargo installs it into ~/.cargo/bin); undo with `just uninstall-cli`.
install-cli:
    cargo install --path crates/debuggatha-cli --locked
    @echo "Installed. Make sure ~/.cargo/bin is on your PATH, then run: debuggatha --version"

uninstall-cli:
    -cargo uninstall debuggatha-cli

# Startup and one file review with the release binary (docs/benchmarks.md has the TypeScript baseline).
bench: build
    target/release/debuggatha --version
    target/release/debuggatha review --files crates/debuggatha-core/src/lib.rs --fail-on none --quiet --no-persist

# Package and verify the crates in publish order without uploading (add --allow-dirty on an uncommitted tree).
rust-publish-dry-run *args:
    cargo publish --workspace --dry-run --locked {{args}}

# --- VS Code extension (a client of the binary) ---------------------------------------------

# Bootstrap the extension's Node toolchain from its lockfile.
vscode-install:
    pnpm --dir {{vscode}} install --frozen-lockfile

# Write src/contracts from docs/schemas (the types of what the daemon sends).
vscode-contracts:
    pnpm --dir {{vscode}} run contracts

vscode-build:
    pnpm --dir {{vscode}} run build

# Run the extension in watch mode.
dev:
    pnpm --dir {{vscode}} run dev

# The extension's gate: contracts up to date, lint, types, and unit tests against the real binary.
vscode-check: build-debug
    pnpm --dir {{vscode}} run contracts:check
    pnpm --dir {{vscode}} run lint
    pnpm --dir {{vscode}} run typecheck
    DEBUGGATHA_CLI='{{justfile_directory()}}/target/debug/debuggatha' pnpm --dir {{vscode}} run test
    pnpm --dir {{vscode}} run build

# The binary the extension's tests drive.
build-debug:
    cargo build -p debuggatha-cli --locked

# Run the extension inside a real VS Code: the oldest one it supports (downloads it on first run; needs a display, so CI uses xvfb).
test-vscode: build-debug
    DEBUGGATHA_CLI='{{justfile_directory()}}/target/debug/debuggatha' pnpm --dir {{vscode}} run test:integration

# The same suite in the current stable VS Code.
test-vscode-stable: build-debug
    DEBUGGATHA_CLI='{{justfile_directory()}}/target/debug/debuggatha' VSCODE_TEST_VERSION=stable pnpm --dir {{vscode}} run test:integration

# Package the extension into a .vsix.
vscode-package: vscode-build
    pnpm --dir {{vscode}} run package

# --- Distribution ---------------------------------------------------------------------------

# The version every published manifest must carry (the Cargo workspace's).
version := `cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "debuggatha-cli") | .version'`

# Every manifest agrees on one version: the crates, the npm launchers and platform packages, the MCP registry manifest and the extension.
verify-version tag=version:
    scripts/verify-version.sh {{tag}}

# What the npm launchers would ship, without publishing.
npm-check: (verify-version version)
    cd npm/cli && npm pack --dry-run --silent
    cd npm/mcp && npm pack --dry-run --silent

# Pack the launchers and this machine's platform package, install them into an empty project, and run what a user would run.
smoke: build
    DEBUGGATHA_BINARY="{{justfile_directory()}}/target/release/debuggatha{{exe}}" node scripts/smoke-pack.mjs

# The release archive of one target, built by `cargo build --release --target "$TARGET"`: TARGET and VERSION in the environment, the archive in dist/.
package:
    #!/usr/bin/env bash
    set -euo pipefail
    : "${TARGET:?the Rust target triple}" "${VERSION:?the version}"
    name="debuggatha-$VERSION-$TARGET"
    stage="dist/$name"
    rm -rf "$stage"; mkdir -p "$stage"
    case "$TARGET" in
        *windows*) cp "target/$TARGET/release/debuggatha.exe" "$stage/" ;;
        *)         cp "target/$TARGET/release/debuggatha" "$stage/" ;;
    esac
    cp LICENSE README.md "$stage/"
    case "$TARGET" in
        *windows*) (cd dist && 7z a -tzip -bso0 "$name.zip" "$name") ;;
        *)         tar -C dist -czf "dist/$name.tar.gz" "$name" ;;
    esac
    rm -rf "$stage"
    ls dist

# The release notes of VERSION, from the Conventional Commits since the previous tag, in RELEASE_NOTES.md.
changelog:
    : "${VERSION:?the version}"
    git cliff --latest --tag "v$VERSION" --strip header --output RELEASE_NOTES.md

# Measure a local model on the labeled set of the semantic layer (needs Ollama; `--runtime lmstudio`, `--model <name>`, `--verbose`).
semantic-eval *args:
    cargo run --release --locked -p debuggatha-engine --example semantic_eval -- {{args}}

# --- Real-repository corpus (opt-in: clones the pinned repositories) ------------------------

# The detectors and the ledger on the real corpus against the goldens in corpus/golden: fetch the pinned repositories, then run the gates.
corpus-gate dest="/tmp/debuggatha-corpus":
    scripts/fetch-corpus.sh {{dest}}
    DEBUGGATHA_CORPUS_DIR={{dest}} cargo test -p debuggatha-core --test corpus_gate -- --ignored --nocapture
    DEBUGGATHA_CORPUS_DIR={{dest}} cargo test -p debuggatha-engine --test corpus --release -- --ignored --nocapture

clean:
    cargo clean
    rm -rf dist
    pnpm --dir {{vscode}} run clean
