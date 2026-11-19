# Architecture

## Overview

Debuggatha is a native Rust engine. One Cargo workspace builds the `debuggatha` binary, which is the CLI, the daemon (Protocol v1) and the MCP server. The VS Code extension and every other adapter are clients of that binary, and they never link the engine. The TypeScript engine that preceded it is frozen in the git history (tag `ts-engine-final`); what it left in the tree is the contract the Rust modules are tested against.

```text
debuggatha/
├── crates/          — the Rust workspace: core, engine, cli
├── apps/vscode/     — VS Code extension (Node + pnpm), a client of the binary
├── npm/             — launchers `@sxnnyside/debuggatha-cli` and `-mcp`: they run the native binary, no engine inside
├── corpus/          — goldens of the TypeScript engine and the pinned real repositories
├── scripts/         — version check, npm packaging, pack smoke test, corpus fetcher, commit checks
├── docs/            — Protocol v1, parity, roadmap, the Review Pack spec, schemas and ADRs
└── Justfile         — the command surface
```

## Components

| Component | Responsibility | Depends on |
|---|---|---|
| `debuggatha-core` | The review domain: `repository_intelligence` (stack, capabilities, dependencies, docs, criteria), `review_engine` (request, session, finding, result), `knowledge_system` (skills, packs, policies), `findings_ledger` (lifecycle, matching, sync), `repository_memory` (suppressions, deviations, decisions) | nothing in the repository |
| `debuggatha-engine` | Runs reviews: `pipeline`, `syntax`, `semantic`, `detect`, `analysis`, `analyzers`, `semantic_layer`, `daemon` (Protocol v1), baseline, scan scope, git and diff. Review Packs, queries and concept tables are data under `data/` | `debuggatha-core` |
| `debuggatha-cli` | The `debuggatha` binary: `commands/`, `mcp/` (tools, prompts, resources, protocol, server, http, input-required), `report/` (text, json, sarif, github, markdown, gitlab), `serve.rs`, `config.rs` | `debuggatha-engine` |
| `npm/cli`, `npm/mcp` | Launchers: find the binary (`DEBUGGATHA_CLI`, the platform package, `PATH`) and run it | the binary |
| `apps/vscode` | Diagnostics, tree views, status bar, hover and quick fixes over the daemon | the binary, over Protocol v1 |

Dependency direction: `core` ← `engine` ← `cli`. The engine is language-agnostic: detectors read a language-neutral semantic model that per-language adapters fill, and rules name concepts that data tables bind to each language.

## Data flow

`pipeline::execute_review` runs these steps:

1. Build the `RepositoryContext` (Core Skills: stack and capabilities, dependencies, documentation, criteria).
2. Request the registered packs; Rule Resolution keeps the rules whose scope matches the detected capabilities and records conflicts.
3. Assemble the `ReviewPolicy` (its id is deterministic, ADR-0005).
4. Run the Review Skill for the scope (workspace, files, or diff) over the project's own code: oxc for JS/TS, tree-sitter for the rest, one parse status per file, detectors over the semantic model.
5. Run the enabled external analyzers as separate processes and merge their findings, labeled with tool, version and license. Each has a trust class (`safe`, `runs-project-code`, `network`); only the person running the review can enable the last two.
6. Apply inline suppressions and active Repository Memory; suppressed findings are reported, never hidden.
7. Synchronize into the Findings Ledger, apply the baseline, and report what the change introduced, fixed and reopened.

The optional model pass runs after the same pipeline. Its claims are kept only when they quote the line they are about, and it never closes, hides or lowers a finding. Code goes only to `localhost` or, over the daemon and MCP, to the connected client's own model (sampling, or `input_required` with `review/resume`).

A review states what it could not check (`coverage`): a language with no parser or a rule with no detector is never reported as clean. The Analysis stage (module boundaries, dependency graph, layers, cycles, public API, dead code, ownership, change impact) is deterministic and model-free; it discovers, it does not judge.

Everything Debuggatha persists lives in `.debuggatha/` at the repository root: `ledger.json`, `memory.json`, `baseline.json` and the optional `config.json`. All are deterministic, human-readable JSON written atomically; versioned files fail closed on a schema version they do not recognize. In this repository the directory is ignored, because Debuggatha reviewing its own source writes its ledger there.

## Decisions

| Decision | Why | Alternatives rejected |
|---|---|---|
| One native Rust engine; the CLI is the engine and adapters are clients (ADR-0006) | Regex detection could not see scope, calls or data flow; the engine existed three times; reviewing a Go or Rust repository needed Node | Keep the TypeScript engine with WebAssembly tree-sitter; a Rust sidecar for parsing only; native JS parsers bound from Node; porting the TypeScript type checker |
| The knowledge-system domain lives in its own module (ADR-0001) | Pack and Policy logic is domain logic, neither content nor orchestration | Logic in `engine/policies`; folding the types into `review-engine` |
| Review Packs are semver-versioned and a reference carries a `version` (ADR-0002) | A finding's originating pack must stay citable after the pack updates | `packId@version` inside `id`; unversioned references; a lookup table of opaque ids |
| Rule conflicts are recorded with a fixed precedence, never merged silently (ADR-0003) | Every finding must trace to a source, and resolution must be deterministic | Last writer wins; refusing to assemble on conflict; detecting contradictions with a model |
| The Capability Registry is an injectable interface with an in-memory default (ADR-0004) | Test isolation and no process boundary to justify a protocol | A global singleton; filesystem pack discovery; capability negotiation over a protocol |
| `ReviewPolicy.id` is a deterministic hash of its inputs (ADR-0005) | Equivalent runs are recognizable and recomputable without state | Random ids; an incrementing id from a central authority; persisting every policy |
| The TypeScript engine is frozen; its fixtures and goldens are the contract | Parity is checked against files the old engine wrote, and they cannot be regenerated | None recorded |

Every intended difference from the TypeScript engine is listed with its reason in [docs/PARITY.md](docs/PARITY.md); a difference not listed there is a bug. The decision records are in [docs/adr/](docs/adr/).

## Constraints

- **The deterministic core never depends on a model.** The semantic layer is off by default and enabled only by the person running the review.
- **A config file can only make a review stricter or quieter.** It can never enable an analyzer that runs project code or uses the network, nor the semantic layer.
- **The ledger fingerprint stays byte-identical** to the TypeScript engine's, or existing ledgers would reopen every finding.
- **Protocol v1** is NDJSON over stdio, bound to one repository root fixed at start ([docs/PROTOCOL.md](docs/PROTOCOL.md)).
- **Platforms:** macOS arm64, Linux x64 and arm64, Windows x64. Intel macOS is not shipped.
- **Secrets** are redacted in excerpts before they reach reports, the ledger or a model; errors reaching an MCP client carry no raw paths or stack traces.
