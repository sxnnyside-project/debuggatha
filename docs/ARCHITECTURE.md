# Architecture

Debuggatha is a Bun + Turborepo monorepo of five packages and one app. The review pipeline lives in one engine; MCP, the CLI, and the VS Code extension are thin adapters over it.

## Structure

```text
debuggatha/
├── apps/
│   └── vscode/              — VS Code extension (Node + pnpm), adapter over @debuggatha/engine
│
├── packages/
│   ├── core/                — the review domain; depends on nothing else in the repo
│   │   └── src/
│   │       ├── repository-intelligence/ — RepositoryContext (stack, capabilities, deps, docs, criteria)
│   │       ├── review-engine/           — review domain (Request/Session/Finding/Result)
│   │       ├── knowledge-system/        — Skills/Packs/Policies domain (Registry, resolvePolicy, assembleContext)
│   │       ├── findings-ledger/         — persistent history of findings (lifecycle, matching, sync)
│   │       ├── repository-memory/       — long-term engineering knowledge (suppressions, deviations, decisions)
│   │       └── testing/                 — fixture helpers, exported as @debuggatha/core/testing
│   ├── packs/               — Review Pack content, conforming to core's ReviewPack schema
│   ├── engine/              — runs reviews; depends on core and packs
│   │   └── src/
│   │       ├── pipeline.ts, provider.ts — executeReview and RepositoryProvider
│   │       ├── skills/                  — Review Skills and rule detectors
│   │       ├── analysis-engine/         — deterministic dependency graph, cycles, layers, dead code, ownership
│   │       ├── analyzers/               — external analyzers run as separate processes
│   │       ├── semantic/                — optional model pass, grounded and loopback-only
│   │       ├── runtime-engine/          — Ollama and LM Studio providers
│   │       ├── policies/                — assemblePolicy against the pack registry
│   │       ├── scan-scope/              — what counts as the project's own code
│   │       ├── baseline.ts              — known findings, for adoption mode
│   │       └── infrastructure/          — LocalGitProvider
│   ├── mcp/                 — MCP server, published on its own
│   └── cli/                 — CLI for terminals and CI, published on its own
│
├── docs/                    — this file, the Review Pack spec, and ADRs
├── scripts/                 — license gate, pack smoke test, release tooling
└── Justfile                 — the command surface for the whole repository
```

Dependency direction: `core` ← `packs` ← `engine` ← (`cli`, `mcp`, `apps/vscode`). `cli` and `mcp` bundle everything they depend on, so what is published depends only on third-party packages. Modules inside a package import each other by relative path through each module's `index.ts`.

## The review pipeline

`executeReview` is `analyzeReview` followed by `concludeReview`:

1. Build the `RepositoryContext` (Core Skills: stack and capabilities, dependencies, documentation, criteria).
2. Request the registered packs; Rule Resolution keeps the rules whose scope matches the detected capabilities and records conflicts.
3. Assemble the `ReviewPolicy` (its id is deterministic, see ADR-0005).
4. Run the Review Skill for the scope (file, diff, or architecture) over the project's own code.
5. Run the enabled external analyzers as separate processes and merge their findings, labeled with tool, version, and license.
6. Apply inline suppressions and active Repository Memory; suppressed findings are reported, never hidden.
7. Synchronize into the Findings Ledger, apply the baseline, and report what the change introduced, fixed, and reopened.

`executeReviewWithSemantics` runs the same pipeline and then an optional model pass whose claims are kept only when they quote the line they are about; the pass never closes, hides, or lowers a finding.

## Subsystems

**Repository Intelligence** (`core/repository-intelligence`): Stack Detection, Dependency Context, Documentation Context, Criteria Resolution, Repository Understanding, the additive `Capability[]` set rules match against, the immutable `RepositoryContext` snapshot, and its fingerprint-based cache.

**Review Engine domain** (`core/review-engine`): `ReviewRequest`, `ReviewSession` with an enforced lifecycle, `Finding` (cannot be constructed without evidence and a location), `Evidence`, `Category`, independent `Severity` and `Confidence`, `Recommendation`, `ReviewResult`, `ReviewSummary`.

**Knowledge System** (`core/knowledge-system`): `SkillDescriptor`, `ReviewPack`/`Rule`/`KnowledgeEntry`, an injectable `CapabilityRegistry`, `resolvePolicy` (pure, fail-closed on missing dependencies), `assembleContext`, `ReviewPolicy`/`ConflictRecord` (conflicts recorded with fixed precedence, never dropped), and `validateReviewPack`.

**Findings Ledger** (`core/findings-ledger`): immutable `Ledger`/`LedgerEntry`, a five-state lifecycle, append-only history, finding identity via `computeFindingFingerprint` (file + rule id + content anchor, never line number alone), a pluggable `FindingMatcher`, `synchronizeReviewResult` (new, matched, auto-resolved, reopened, scoped to what a review covered), and deterministic versioned persistence at `.debuggatha/ledger.json`, written atomically.

**Repository Memory** (`core/repository-memory`): structured engineering decisions with their own lifecycle; only active items filter findings.

**Analysis Engine** (`engine/analysis-engine`): a deterministic, model-free pipeline over module boundaries, dependency graph, layer model and violations, cycles, public API surface, dead code, ownership, and change impact. Architecture Review consumes it.

**Analyzers** (`engine/analyzers`): adapters for external tools, each with a trust class (`safe`, `runs-project-code`, `network`). Only the person running the review can enable the last two.

**Semantic layer** (`engine/semantic`, `engine/runtime-engine`): optional, off by default, loopback-only, grounded.

**Engine** (`@debuggatha/engine`) is the one entry point for the adapters: `executeReview` plus the public surface of `core` they need.

## Local state

Everything Debuggatha persists lives in `.debuggatha/` at the repository root: `ledger.json`, `memory.json`, `baseline.json`, and the optional `config.json`. All are deterministic, human-readable JSON; versioned files fail closed on a schema version they do not recognize.
