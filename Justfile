# Command surface for the whole repository.
# Bun drives everything under packages/ and apps/docs; the VS Code extension
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

# Verify formatting without writing (used by check and CI).
format-check:
    bunx biome check --linter-enabled=false .

# Full quality gate. CI runs exactly this.
check: format-check lint typecheck test build

# Package the extension into a .vsix.
package: build
    pnpm --dir {{vscode}} run package

clean:
    bun run clean
    pnpm --dir {{vscode}} run clean
