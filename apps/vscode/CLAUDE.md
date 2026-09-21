# apps/vscode — CLAUDE.md

The Debuggatha VS Code extension. Unlike the rest of the repository, this package is **Node +
pnpm**, not Bun: the VS Code extension host runs on Node, and Marketplace packaging (`vsce`)
expects a Node toolchain.

## Toolchain

| Concern | Tool |
| --- | --- |
| Runtime / package manager | Node (`.node-version`), pnpm (pinned in `packageManager`) |
| Bundle | esbuild via `esbuild.config.mjs` → `dist/index.js` (CJS, `vscode` external) |
| Correctness | TypeScript strict (shared `tsconfig.base.json`) |
| Format / lint | Biome (shared root `biome.json`) |
| Unit tests | Vitest (`vi.mock("vscode")` is why it stays Vitest, not `bun test`) |
| Packaging | `vsce package --no-dependencies` |

## Rules

- Never add this directory to the root `workspaces` in `package.json`, and never use Bun here.
- `@debuggatha/engine` is a `link:` dev dependency (`../../packages/engine`) and is bundled into
  `dist/index.js`. Packages must be built first (`just build-packages`) for typecheck to
  resolve their types; the root `just` recipes already order this.
- Commit `pnpm-lock.yaml`. pnpm build scripts are allow-listed in `pnpm-workspace.yaml`
  (`allowBuilds`); add to it deliberately.
- Run everything through the root `Justfile` (`just dev`, `just check`, `just package`).
  Direct equivalents: `pnpm run <build|dev|lint|typecheck|test|package>` from this directory.
- Business logic lives in `packages/*`; keep this layer to VS Code wiring (commands, views,
  diagnostics, status bar).
- Reviews run in a separate worker process (`dist/review-worker.js`), never in the extension host.
- The extension runs only built-in detectors in an untrusted workspace, and analyzer and local-model settings are machine scope; do not weaken either.
- Tests: vitest for units (`src/test/vscode-mock.ts`), and mocha + `@vscode/test-electron` in a real VS Code (`just test-vscode`, `just test-vscode-stable`).
