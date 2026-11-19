# apps/vscode — CLAUDE.md

The Debuggatha VS Code extension. Unlike the rest of the repository, this package is **Node +
pnpm**, not Bun: the VS Code extension host runs on Node, and Marketplace packaging (`vsce`)
expects a Node toolchain.

## Toolchain

| Concern                   | Tool                                                                                                          |
| ------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Runtime / package manager | Node (`.node-version`), pnpm (pinned in `packageManager`)                                                     |
| Bundle                    | esbuild via `esbuild.config.mjs` -> `dist/index.js` (CJS, `vscode` external; the extension host requires CJS) |
| Correctness               | TypeScript strict (shared `tsconfig.base.json`), `types` set explicitly                                       |
| Format / lint             | Biome (shared root `biome.json`)                                                                              |
| Unit tests                | Vitest, driving the real `debuggatha` binary (`vi.mock("vscode")` is why it stays Vitest, not `bun test`)     |
| Contracts                 | `scripts/contracts.mjs` writes `src/contracts/*.ts` from `docs/schemas` (`json-schema-to-typescript`)         |
| Packaging                 | `vsce package --no-dependencies`; Open VSX with `ovsx publish` (both are dev dependencies)                    |

## Rules

- This directory is its own Node + pnpm project; never use Bun here.
- The extension is a client of the `debuggatha` binary over Protocol v1 (`docs/PROTOCOL.md`). It
  imports no engine code and bundles none: `src/daemon/` starts `debuggatha serve`,
  does the handshake, checks the version (`compat.ts`) and speaks NDJSON. Every call in `daemon/api.ts`
  is one protocol method; keep decisions out of it.
- Types of what the daemon sends are generated: change `docs/schemas`, run `just vscode-contracts`,
  commit the result. `src/contracts/index.ts` is the only file that imports the generated ones.
- What a person alone may enable is decided when the daemon starts (`daemon/connection.ts`):
  analyzers that run code or use the network, the local model, and the binary's path come from
  machine-scope settings, and an untrusted workspace gets none of them. A request can only narrow.
- Commit `pnpm-lock.yaml`. pnpm build scripts are allow-listed in `pnpm-workspace.yaml`
  (`allowBuilds`); add to it deliberately. `@types/vscode` is pinned to `engines.vscode`.
- Run everything through the root `Justfile` (`just dev`, `just vscode-check`, `just vscode-package`).
  Direct equivalents: `pnpm run <build|dev|lint|typecheck|test|contracts|package>` from this directory.
- Business logic lives in the engine; keep this layer to VS Code wiring (commands, views,
  diagnostics, status bar).
- The extension runs only built-in detectors in an untrusted workspace, and analyzer and local-model settings are machine scope; do not weaken either.
- Tests: vitest for units (`src/test/vscode-mock.ts`) against a real daemon (`cargo build -p debuggatha-cli` makes the binary; the global setup does it), and mocha + `@vscode/test-electron` in a real VS Code (`just test-vscode`, `just test-vscode-stable`).
