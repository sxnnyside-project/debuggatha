# Command surface for the whole repository.
# The native engine (Cargo workspace under crates/) is the product: the recipes without a prefix
# drive it. What is left of the TypeScript engine is the parity reference (ADR-0006) and sits under
# `parity-*` until the cutover; the VS Code extension (apps/vscode, Node + pnpm) is still the
# consumer and keeps its own recipes.

vscode := "apps/vscode"
host_os := if os() == "macos" { "darwin" } else if os() == "windows" { "windows" } else { "linux" }
host_arch := if arch() == "aarch64" { "arm64" } else { "x64" }
exe := if os() == "windows" { ".exe" } else { "" }

default:
    @just --list

# --- Native engine -------------------------------------------------------------------------

# Fetch the crates the lockfile names (the toolchain is pinned by rust-toolchain.toml).
install:
    cargo fetch --locked

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

# Native gate: format, clippy with warnings denied, tests, dependency policy (needs cargo-deny).
rust-check: format-check lint test
    cargo deny check

# Full quality gate. CI runs exactly this: the native gate and the parity reference's.
check: rust-check parity-check

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

# Startup and one file review with the release binary (docs/benchmarks.md compares it with the TypeScript engine).
bench: build
    target/release/debuggatha --version
    target/release/debuggatha review --files crates/debuggatha-core/src/lib.rs --fail-on none --quiet --no-persist

# Package and verify the crates in publish order without uploading (add --allow-dirty on an uncommitted tree).
rust-publish-dry-run *args:
    cargo publish --workspace --dry-run --locked {{args}}

# --- Parity reference: the TypeScript engine ---------------------------------------------------

# Bootstrap the JavaScript toolchains from their lockfiles.
parity-install:
    bun install --frozen-lockfile
    pnpm --dir {{vscode}} install --frozen-lockfile

parity-build-packages:
    bun run build

parity-build: parity-build-packages
    pnpm --dir {{vscode}} run build

parity-test: parity-build-packages
    bun run test
    pnpm --dir {{vscode}} run test

parity-typecheck: parity-build-packages
    bun run typecheck
    pnpm --dir {{vscode}} run typecheck

parity-lint:
    bun run lint
    pnpm --dir {{vscode}} run lint

# Apply formatting and import organization to the JavaScript side.
parity-format:
    bunx biome check --write --linter-enabled=false .

parity-format-check:
    bunx biome check --linter-enabled=false .

# Licenses of what ships, and proof that no analyzer is bundled or depended on.
parity-licenses: parity-build-packages
    node scripts/license-gate.mjs

# The TypeScript gate: what keeps the reference green while the fixtures are regenerated from it.
parity-check: parity-format-check parity-lint parity-typecheck parity-test parity-build parity-licenses

# Pack the CLI and MCP tarballs, install them into an empty project, and run the installed bins.
smoke: parity-build-packages
    node scripts/smoke-pack.mjs

# Standalone TypeScript executables for every supported platform into dist/binaries (what the release workflow still ships).
parity-binaries: parity-build-packages
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p dist/binaries
    for target in darwin-arm64 linux-x64 linux-arm64 windows-x64; do
        ext=""; [ "${target%%-*}" = "windows" ] && ext=".exe"
        bun build --compile --minify --target="bun-$target" packages/cli/src/bin.ts \
            --outfile "dist/binaries/debuggatha-$target$ext"
    done

parity-bench: parity-build-packages
    bun packages/cli/src/bin.ts --version
    bun packages/cli/src/bin.ts review --files packages/core/src/index.ts --fail-on none --quiet --no-persist

# Measure a local model on the labeled set of the semantic layer (needs Ollama; add `--provider lmstudio`, `--model <name>`, `--verbose`).
semantic-eval *args:
    bun packages/engine/src/semantic/eval/run.ts {{args}}

# --- VS Code extension (the consumer) -------------------------------------------------------

# Rebuild the packages, then run the extension in watch mode.
dev: parity-build-packages
    pnpm --dir {{vscode}} run dev

# Run the extension inside a real VS Code: the oldest one it supports (downloads it on first run; needs a display, so CI uses xvfb).
test-vscode: parity-build-packages
    pnpm --dir {{vscode}} run test:integration

# The same suite in the current stable VS Code.
test-vscode-stable: parity-build-packages
    VSCODE_TEST_VERSION=stable pnpm --dir {{vscode}} run test:integration

# Package the extension into a .vsix.
package: parity-build
    pnpm --dir {{vscode}} run package

# --- Parity against the real corpus ---------------------------------------------------------

# The phase 2 gate on the real corpus: fetch the pinned repositories, then compare the Rust engine with the TypeScript goldens.
corpus-gate dest="/tmp/debuggatha-corpus":
    node scripts/parity/fetch-corpus.mjs {{dest}}
    DEBUGGATHA_CORPUS_DIR={{dest}} cargo test -p debuggatha-core --test corpus_gate -- --ignored --nocapture

# The detector port on the real corpus: groups A to E detector by detector against the TypeScript detectors (needs the clones and bun), then the gate: every detector together against the golden findings.
corpus-detectors dest="/tmp/debuggatha-corpus":
    node scripts/parity/fetch-corpus.mjs {{dest}}
    DEBUGGATHA_CORPUS_DIR={{dest}} cargo test -p debuggatha-engine --test corpus_parity_js_syntax --release -- --ignored --nocapture
    bun scripts/parity/dump-detector-hits.ts {{dest}} {{dest}}/ts-hits-js-calls.json no-async-foreach no-shell-interpolation no-open-cors no-error-leak async-route-errors no-focused-tests no-empty-tests no-assertionless-tests
    DEBUGGATHA_CORPUS_DIR={{dest}} DEBUGGATHA_TS_HITS={{dest}}/ts-hits-js-calls.json cargo test -p debuggatha-engine --test corpus_parity_js_calls --release -- --ignored --nocapture
    bun scripts/parity/dump-detector-hits.ts {{dest}} {{dest}}/ts-hits-tree-sitter-rules.json no-unwrap-expect no-unsafe-blocks no-panic avoid-print no-eval
    DEBUGGATHA_CORPUS_DIR={{dest}} DEBUGGATHA_TS_HITS={{dest}}/ts-hits-tree-sitter-rules.json cargo test -p debuggatha-engine --test corpus_parity_tree_sitter_rules --release -- --ignored --nocapture
    bun scripts/parity/dump-detector-hits.ts {{dest}} {{dest}}/ts-hits-vue-template.json use-v-html-carefully
    DEBUGGATHA_CORPUS_DIR={{dest}} DEBUGGATHA_TS_HITS={{dest}}/ts-hits-vue-template.json cargo test -p debuggatha-engine --test corpus_parity_vue_template --release -- --ignored --nocapture
    bun scripts/parity/dump-detector-hits.ts {{dest}} {{dest}}/ts-hits-secrets.json no-hardcoded-secrets
    DEBUGGATHA_CORPUS_DIR={{dest}} DEBUGGATHA_TS_HITS={{dest}}/ts-hits-secrets.json cargo test -p debuggatha-engine --test corpus_parity_secrets --release -- --ignored --nocapture
    DEBUGGATHA_CORPUS_DIR={{dest}} cargo test -p debuggatha-engine --test corpus_gate --release -- --ignored --nocapture
    DEBUGGATHA_CORPUS_DIR={{dest}} cargo test -p debuggatha-engine --test corpus_recall --release -- --ignored --nocapture

# The analysis engine on the real corpus: boundaries, graph, layers, cycles, public API, dead code, ownership and impact against the TypeScript analysis (needs the clones and bun).
corpus-analysis dest="/tmp/debuggatha-corpus":
    node scripts/parity/fetch-corpus.mjs {{dest}}
    bun scripts/parity/dump-analysis.ts {{dest}} {{dest}}/ts-analysis.json
    DEBUGGATHA_CORPUS_DIR={{dest}} DEBUGGATHA_TS_ANALYSIS={{dest}}/ts-analysis.json cargo test -p debuggatha-engine --test corpus_graph --release -- --ignored --nocapture
    DEBUGGATHA_CORPUS_DIR={{dest}} DEBUGGATHA_TS_ANALYSIS={{dest}}/ts-analysis.json cargo test -p debuggatha-engine --test corpus_module_api --release -- --ignored --nocapture

clean:
    cargo clean
    rm -rf dist

# Also remove the JavaScript build outputs.
parity-clean: clean
    bun run clean
    pnpm --dir {{vscode}} run clean
