# Architecture

Debuggatha is a native Rust engine. One Cargo workspace builds the `debuggatha` binary, which is the CLI, the daemon (Protocol v1) and the MCP server. The VS Code extension and every other adapter are clients of that binary. The TypeScript engine that preceded it is kept as the **parity reference** until the cutover (ADR-0006, [ROADMAP.md](ROADMAP.md)): fixes and parity adjustments only.

## Structure

```text
debuggatha/
├── crates/
│   ├── debuggatha-core/     — the review domain; depends on nothing else in the repo
│   │   └── src/
│   │       ├── repository_intelligence/ — RepositoryContext (stack, capabilities, deps, docs, criteria)
│   │       ├── review_engine/           — Request/Session/Finding/Result
│   │       ├── knowledge_system/        — Skills/Packs/Policies (registry, policy resolution, deterministic ids)
│   │       ├── findings_ledger/         — persistent history of findings (lifecycle, matching, sync)
│   │       └── repository_memory/       — long-term engineering knowledge (suppressions, deviations, decisions)
│   ├── debuggatha-engine/   — runs reviews; depends on core
│   │   ├── data/                        — Review Packs, tree-sitter queries, concept tables, patterns
│   │   └── src/
│   │       ├── pipeline.rs              — the review pipeline
│   │       ├── syntax/                  — oxc for JS/TS, tree-sitter for the rest, parse status per file
│   │       ├── semantic/                — the language-neutral semantic model the adapters fill
│   │       ├── detect/                  — detectors over the semantic model; coverage of what could not be checked
│   │       ├── analysis/                — module boundaries, dependency graph, layers, cycles, public API, dead code, ownership, impact
│   │       ├── analyzers/               — external analyzers run as separate processes
│   │       ├── semantic_layer/          — optional model pass, grounded
│   │       ├── daemon.rs, daemon_host.rs — Protocol v1 and the client's own model
│   │       └── baseline.rs, scan_scope.rs, git.rs, diff*.rs
│   └── debuggatha-cli/      — the `debuggatha` binary: commands, `serve` (daemon), `mcp` (stdio and HTTP)
│
├── apps/vscode/             — VS Code extension (Node + pnpm), the consumer of the engine
├── corpus/                  — golden outputs of the TypeScript engine and the pinned real repositories
├── packages/                — the TypeScript engine: parity reference, and what the published packages bundle until the cutover
├── scripts/parity/          — one-off tools that write the conformance fixtures and goldens from the TypeScript engine
├── scripts/                 — license gate, pack smoke test, commit checks
├── docs/                    — this file, Protocol v1, parity, the roadmap, the Review Pack spec, and ADRs
└── Justfile                 — the command surface; `parity-*` recipes drive the TypeScript side
```

Dependency direction: `core` ← `engine` ← `cli`. Adapters never link the engine; they speak Protocol v1 to the binary ([PROTOCOL.md](PROTOCOL.md)). The engine is language-agnostic by design: detectors read a language-neutral semantic model that per-language adapters fill, and rules name concepts that data tables bind to each language.

## The review pipeline

`pipeline::execute_review` runs these steps:

1. Build the `RepositoryContext` (Core Skills: stack and capabilities, dependencies, documentation, criteria).
2. Request the registered packs; Rule Resolution keeps the rules whose scope matches the detected capabilities and records conflicts.
3. Assemble the `ReviewPolicy` (its id is deterministic, see ADR-0005).
4. Run the Review Skill for the scope (workspace, files, or diff) over the project's own code.
5. Run the enabled external analyzers as separate processes and merge their findings, labeled with tool, version, and license.
6. Apply inline suppressions and active Repository Memory; suppressed findings are reported, never hidden.
7. Synchronize into the Findings Ledger, apply the baseline, and report what the change introduced, fixed, and reopened.

The optional model pass runs after the same pipeline; its claims are kept only when they quote the line they are about, and it never closes, hides, or lowers a finding. A review states what it could not check (`coverage`): a language with no parser or a rule with no detector is never reported as clean.

## Subsystems

**Repository Intelligence** (`core::repository_intelligence`): Stack Detection, Dependency Context, Documentation Context, Criteria Resolution, Repository Understanding, the additive `Capability[]` set rules match against, and the immutable `RepositoryContext` snapshot.

**Review Engine domain** (`core::review_engine`): `ReviewRequest`, `ReviewSession` with an enforced lifecycle, `Finding` (cannot be constructed without evidence and a location), independent `Severity` and `Confidence`, `Recommendation`, `ReviewResult`.

**Knowledge System** (`core::knowledge_system`): skills, `ReviewPack`/`Rule`/`KnowledgeEntry`, policy resolution (pure, fail-closed on missing dependencies), `ReviewPolicy` and `ConflictRecord` (conflicts recorded with fixed precedence, never dropped), pack validation. Packs are data under `crates/debuggatha-engine/data/packs`.

**Findings Ledger** (`core::findings_ledger`): a five-state lifecycle, append-only history, finding identity from file + rule id + content anchor (never line number alone), matching, synchronization scoped to what a review covered, and deterministic versioned persistence at `.debuggatha/ledger.json`, written atomically.

**Repository Memory** (`core::repository_memory`): structured engineering decisions with their own lifecycle; only active items filter findings.

**Syntax layer** (`engine::syntax`, `engine::semantic`): oxc for the JavaScript and TypeScript family, tree-sitter for the other languages, a parse status per file, and the semantic model (scopes, calls, data flow) built from either.

**Detectors** (`engine::detect`): rules over the semantic model, including environment reads, taint, promises, secrets and test hygiene; `coverage` records what each review could not check.

**Analysis** (`engine::analysis`): a deterministic, model-free pipeline over module boundaries, dependency graph, layer model and violations, cycles, public API surface, dead code, ownership, and change impact.

**Analyzers** (`engine::analyzers`): adapters for external tools, each with a trust class (`safe`, `runs-project-code`, `network`). Only the person running the review can enable the last two.

**Semantic layer** (`engine::semantic_layer`, `engine::daemon_host`): optional, off by default; code goes only to `localhost` or, over the daemon and MCP, to the connected client's own model (sampling, or `input_required` with `review/resume`).

**Daemon and MCP** (`engine::daemon`, `cli::serve`, `cli::mcp*`): `debuggatha serve` speaks Protocol v1 over stdio; `debuggatha mcp` maps tools, prompts and resources onto the same methods, over stdio and stateless Streamable HTTP on a loopback address.

## Parity reference

`packages/*` (TypeScript, Bun) still run: they are the reference that conformance fixtures and goldens come from, and what the published npm packages bundle until the cutover. Rust modules are validated against them by differential tests; every intended difference is listed with its reason in [PARITY.md](PARITY.md). The one-off tools that write the fixtures are in `scripts/parity/`, and the real-repository comparisons run through `just corpus-gate`, `just corpus-detectors` and `just corpus-analysis`. New syntax-tree capabilities are written once, in Rust.

## Local state

Everything Debuggatha persists lives in `.debuggatha/` at the repository root: `ledger.json`, `memory.json`, `baseline.json`, and the optional `config.json`. All are deterministic, human-readable JSON; versioned files fail closed on a schema version they do not recognize. In this repository the directory is ignored, because Debuggatha reviewing its own source writes its ledger there.
