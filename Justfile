# Command surface for the whole repository.
# Bun drives everything under packages/; the VS Code extension
# (apps/vscode) is Node + pnpm because the extension host requires Node.

vscode := "apps/vscode"

default:
    @just --list

# Bootstrap every toolchain from the lockfiles.
install:
    bun install --frozen-lockfile
    pnpm --dir {{vscode}} install --frozen-lockfile

# Rebuild packages, then run the extension in watch mode.
dev: build-packages
    pnpm --dir {{vscode}} run dev

build-packages:
    bun run build

build: build-packages
    pnpm --dir {{vscode}} run build

test: build-packages
    bun run test
    pnpm --dir {{vscode}} run test

typecheck: build-packages
    bun run typecheck
    pnpm --dir {{vscode}} run typecheck

lint:
    bun run lint
    pnpm --dir {{vscode}} run lint

# Apply formatting and import organization.
format:
    bunx biome check --write --linter-enabled=false .

# Run the extension inside a real VS Code: the oldest one it supports (downloads it on first run; needs a display, so CI uses xvfb).
test-vscode: build-packages
    pnpm --dir {{vscode}} run test:integration

# The same suite in the current stable VS Code.
test-vscode-stable: build-packages
    VSCODE_TEST_VERSION=stable pnpm --dir {{vscode}} run test:integration

# Verify formatting without writing (used by check and CI).
format-check:
    bunx biome check --linter-enabled=false .

# Licenses of what ships, and proof that no analyzer is bundled or depended on.
licenses: build-packages
    node scripts/license-gate.mjs

# Measure a local model on the labeled set of the semantic layer (needs Ollama; add `--provider lmstudio`, `--model <name>`, `--verbose`).
semantic-eval *args:
    bun packages/engine/src/semantic/eval/run.ts {{args}}

# Full quality gate. CI runs exactly this.
check: format-check lint typecheck test build licenses

# Pack the CLI and MCP tarballs, install them into an empty project, and run the installed bins.
smoke: build-packages
    node scripts/smoke-pack.mjs

# Standalone CLI executables for every supported platform into dist/binaries.
binaries: build-packages
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p dist/binaries
    for target in darwin-arm64 darwin-x64 linux-x64 linux-arm64 windows-x64; do
        ext=""; [ "${target%%-*}" = "windows" ] && ext=".exe"
        bun build --compile --minify --target="bun-$target" packages/cli/src/bin.ts \
            --outfile "dist/binaries/debuggatha-$target$ext"
    done

# Package the extension into a .vsix.
package: build
    pnpm --dir {{vscode}} run package

clean:
    rm -rf dist
    bun run clean
    pnpm --dir {{vscode}} run clean
