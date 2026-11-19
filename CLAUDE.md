# Debuggatha — CLAUDE.md

Guide for contributors and coding agents working in this repository.

## What Debuggatha is

A senior code reviewer, not a chat wrapper. Debuggatha understands a repository's context (its
stack, its architecture, its own stated rules) before it reviews anything, and every finding it
produces traces back to a real source.

Thesis: **"A senior reviewer that understands repository context, applies explicit criteria, and
produces traceable findings."** This should guide architecture, UX, and feature decisions
regardless of which model is fashionable.

Debuggatha never writes a user's code. Claude Code, Copilot, or Cursor write; Debuggatha reviews.
It runs as an MCP server, a CLI, and a VS Code extension over one engine.

## Non-negotiables

- **Never opine without evidence.** Every finding cites at least one of: a rule from the repository
  itself, a Review Pack, a language/framework convention, a recognized standard (OWASP, WCAG...),
  the output of an installed analyzer, or evidence in the analyzed code. Never "I think...";
  always "This finding exists because...". No source, no finding.
- **The deterministic core does not depend on a model.** Core Skills, detectors, analyzers, the
  ledger, and every finding a review guarantees must work with no model at all. A language model is
  an optional extra; it never closes, hides, or lowers a finding.
- **MCP is plumbing, not the pitch.** Debuggatha is a specialist an agent calls. Never design or
  market it as "an MCP server" or "another AI chat."
- **One fixed identity.** No personality selector.
- **Trust boundaries are not negotiable** (see below).

## Identity

- Voice: direct, precise, never condescending; a senior reviewer who read the code before opening
  its mouth.
- Never opines cold: orients first (Core Skills), then states its understanding of the project in
  one line before any judgment.
- Never fakes certainty: if it cannot detect something, it says so instead of guessing.

## Trust boundaries

- **External analyzers** run as separate processes and are never bundled or depended on. Each one
  has a trust class: `safe` (only reads source), `runs-project-code` (ESLint, PHPStan, Clippy), or
  `network` (OSV-Scanner). The last two run only when the person enables them (a flag, an
  environment variable, or user-scope settings). A repository file (`.debuggatha/config.json`,
  workspace settings) or an MCP tool call can never enable them.
- **The semantic layer** is off by default and enabled only by the person running the review. Code
  goes only to `localhost` (or, over MCP, to the connected client's own model through sampling).
  Lines that may hold a secret and secret files are never sent. A claim is kept only when it quotes
  the line it is about and the quote is on that line.
- **Secrets** are redacted in excerpts before they reach reports, the ledger, or a model.
- **Git refs** are validated and passed to git without a shell.
- **Errors** reaching an MCP client are sanitized: no raw paths or stack traces.
- **The extension** runs only built-in detectors in an untrusted workspace, and its analyzer and
  local-model settings are machine scope.
- **A config file can only make a review stricter or quieter**, never enable something that runs
  code, uses the network, or sends code to a model.

## Skill taxonomy

Three tiers. Do not flatten them into one list.

**Core Skills** always run, are deterministic, and are model-independent. Each claim carries an
evidence trail, not just a boolean. They live in `core/repository-intelligence`.

- **Stack Detection** reads manifests and resolves ambiguity by content, not filename alone (a
  `Cargo.toml` is only "Tauri" if it depends on tauri). The result is an additive `Capability[]`
  (`typescript`, `react`, `bun`...), each with confidence, evidence, and origin.
- **Criteria Resolution** (not "Criteria Loading") builds a model of the project's criteria from
  every signal: docs (ARCHITECTURE.md, CONTRIBUTING.md, CLAUDE.md), config surfaces (.eslintrc,
  rustfmt.toml, biome.json, detekt.yml, .editorconfig), and what manifests imply.
- **Documentation Context** reads README, ADRs, and roadmaps for intent, not just structure.
- **Repository Understanding** raises a question only when a concrete gap exists; it never invents
  an answer.

**Review Skills** produce judgment, per scope: file review, diff review, and architecture review.
They share one detector engine that turns a resolved `ReviewPolicy` and source units into citable
findings.

**Analysis Skills** discover, they do not judge: module boundaries, dependency graph, layer model
and violations, cycles, public API surface, dead code, ownership, and change impact
(`engine/analysis-engine`). Architecture Review consumes them.

## Review Packs

```
Review Pack
├── Skills     — capabilities the pack activates
├── Policies   — assembled, citable ruleset for a given review
├── Rules      — atomic, individually citable statements
└── Knowledge  — underlying domain expertise the rules derive from
```

- Stack packs (Rust, Flutter, React, Tauri, Kotlin...) and concern packs (OWASP, Accessibility,
  Performance, DX, Release...). The spec is [docs/PACK_SPEC.md](docs/PACK_SPEC.md); content lives in
  `packages/packs`.
- A **Review Policy** is a pack's Rules plus the repository's own resolved criteria, assembled for
  one review run. It is always inspectable and citable. Its id is deterministic (ADR-0005).
- Conflicts between rules are recorded with a fixed precedence, never silently dropped (ADR-0003).
- Rules match against the capability set, not exact stack strings.

## Findings Ledger and memory

- The ledger (`.debuggatha/ledger.json`) persists findings across sessions and clients: a stable
  identity (file + rule id + content anchor, never line number alone), a five-state lifecycle
  (open, acknowledged, resolved, dismissed, reopened) enforced as a state machine, and an
  append-only history. Writes are atomic. Findings no longer detected within the reviewed scope
  resolve; resolved ones that reappear reopen. A finding from a tool or model that did not run is
  not called fixed.
- Repository Memory (`.debuggatha/memory.json`) keeps structured engineering decisions: accepted
  deviations, suppressions, exceptions, conventions. It has its own lifecycle (detected, suggested,
  confirmed, active, deprecated, archived); only `active` items ever filter findings, and only a
  person can confirm one. Suppressed findings are reported, never silently hidden.
- The baseline (`.debuggatha/baseline.json`) records what a repository has today so later reviews
  report only what is new. The ledger still tracks everything.
- The ledger is a persistent record of findings; `RepositoryContextCache` is an in-process cache of
  the repository snapshot. Different subsystems, different lifetimes.

## Vocabulary

| Don't say | Say | Why |
|---|---|---|
| Prompt | Skill | Not text pasted to a model, but something Debuggatha knows how to do. |
| Templates / Knowledge Packs | Review Pack | Consumed to review, not just to "know." |
| Prompt (per run) | Review Policy | The auditable criteria behind a specific finding. |
| Criteria Loading | Criteria Resolution | Resolving a model from every signal, not loading markdown. |
| Personality (plural) | Identity (singular) | One fixed voice. |

## What NOT to do

- Do not bring back a personality selector.
- Do not let a Review Skill run before Core Skills have oriented on the repo.
- Do not ship a finding with no citable source.
- Do not make a Core Skill or a guaranteed finding depend on a model provider.
- Do not let a repository file or a tool call enable an analyzer that runs code or uses the network,
  or the semantic layer.
- Do not put business logic in an adapter.
- Do not market or design around "MCP server" as the pitch.

## Direction: native Rust engine (ADR-0006, accepted)

The engine is moving from TypeScript to a native Rust workspace (`debuggatha-core`,
`debuggatha-engine`, `debuggatha-cli`) that is the single engine; the CLI, MCP server and editor
extensions become clients of it over Protocol v1 ([docs/PROTOCOL.md](docs/PROTOCOL.md)). The plan,
gates and measured spike results are in [docs/ROADMAP.md](docs/ROADMAP.md); contracts are in
[docs/schemas/](docs/schemas/README.md).

What this means while the migration is in progress:

- **Everything below describes the code as it is today.** The TypeScript engine stays the shipped
  engine until the cutover gate, and it is kept in step as the **parity reference**: fix bugs and
  keep its tests green, but do not build new syntax-tree capabilities in TypeScript. Those are
  written once, in Rust, and validated against the fixtures and the corpus in `corpus/`.
- **The core is language-agnostic by design.** No detector or analysis is written against one
  language's syntax tree: they read a language-neutral semantic model that per-language adapters fill, and rules
  name concepts that data tables bind to each language. JavaScript is one adapter among many; parity with the
  TypeScript engine is a regression check for JavaScript, never the scope of a stage.
- **No regex is added to stand in for a parser.** A rule that needs scope, call structure or data
  flow waits for the syntax layer; it is not approximated.
- **A review must say what it could not check.** "0 findings" never stands for "clean" when a
  language had no parser or a rule had no detector.
- **Parity is differential.** A ported module must give identical findings on the golden corpus
  (`corpus/golden/`) unless a divergence is listed with its reason. The ledger fingerprint must be
  byte-identical.
- JS/TS use `oxc`; other languages use tree-sitter. Packs are JSON (TOML for authoring).
- When a rule above conflicts with this direction (for example, the semantic layer over MCP, below),
  the trust boundary stays and the mechanism changes; see ADR-0006 and PROTOCOL.md section 7.

## Repository layout and tooling

Two toolchains behind one command surface:

- `packages/*` use **Bun** (workspaces, Turborepo, tsup, `bun test`, Biome).
- `apps/vscode` uses **Node + pnpm** (esbuild, vitest, `vsce`). It is deliberately not a Bun
  workspace member; read [apps/vscode/CLAUDE.md](apps/vscode/CLAUDE.md) before touching it.

Package map ([docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) has the detail):

- `core` is the review domain and depends on nothing: `repository-intelligence`, `review-engine`,
  `knowledge-system`, `findings-ledger`, `repository-memory`.
- `packs` is rule content.
- `engine` runs reviews: `executeReview` (analyze, then conclude), `executeReviewWithSemantics`,
  `skills`, `analysis-engine`, `analyzers`, `semantic`, `policies`, baseline, and scan scope.
- `cli`, `mcp`, and `apps/vscode` are thin adapters. `cli` and `mcp` are published separately and
  bundle their internal dependencies.

Adapters import from `@debuggatha/engine` only. Inside a package, modules import each other by
relative path through each module's `index.ts`. Each module's README records its design decisions
and limitations.

Everything is driven from the root `Justfile`: `just install | dev | build | test | typecheck |
lint | format | format-check | check | bench | smoke | test-vscode | test-vscode-stable | licenses |
semantic-eval | package | binaries | clean`. `just check` is the full gate and is exactly what CI runs.
Prefer `just` over calling `bun` or `pnpm` directly.

Hooks (Husky): pre-commit runs `just check` (the full quality gate), commit-msg enforces
Conventional Commits.

Domain glossary: see [docs/glossary.md](docs/glossary.md).

## Where things go

| Kind | Location | Naming |
|---|---|---|
| Core intelligence & domain | `packages/core/src/` | kebab-case modules, PascalCase types |
| Review rule packs | `packages/packs/src/packs/` | kebab-case pack files |
| Review execution & analyzers | `packages/engine/src/` | kebab-case directories, index.ts entry |
| CLI commands & adapters | `packages/cli/src/commands/` | kebab-case command files |
| MCP tools & server | `packages/mcp/src/tools/` | kebab-case tool handlers |
| VS Code extension | `apps/vscode/src/` | kebab-case modules |
| Tests | Beside source or `tests/` | `<name>.test.ts`, `<name>.itest.ts` |
| Scripts | `scripts/` | kebab-case scripts (`.sh`, `.mjs`) |
| Docs & ADRs | `docs/`, `docs/adr/`, `docs/glossary.md` | kebab-case or SCREAMING_SNAKE |

## Interfaces

- **MCP** (`@sxnnyside/debuggatha-mcp`): review tools (`review_changes`, `review_diff`, `review_files`,
  `review_workspace`), ledger tools (`list_findings`, `get_finding`, `update_finding`,
  `explain_finding`, `suppress_finding`, `create_baseline`), and context tools
  (`repository_context`, `repository_summary`, `list_analyzers`). Stdio and Streamable HTTP.
  Prompts: `review-flow`, `fix-findings`, `triage-findings`. Every handler delegates to the engine.
- **CLI** (`@sxnnyside/debuggatha-cli`): `review`, `baseline`, `findings`, `memory`, `repository`, `packs`,
  `policies`, `analyzers`, `init`, `trends`, `doctor`, `completion`. Exit codes: `0` clean, `2`
  findings at or above the threshold, `1` error.
- **VS Code** (`apps/vscode`): native components only (diagnostics, tree views, status bar, hover,
  quick fixes). Reviews run in a separate process.

## Model strategy

- Default: no model. Deterministic detectors plus installed analyzers.
- Optional semantic pass: a local runtime (Ollama, LM Studio) or, over MCP, the connected client's
  own model through sampling. See the trust boundaries above. Sampling is deprecated in MCP
  2026-07-28, which forbids server-to-client requests; the Rust engine adds an input-required and
  resume path for those hosts (PROTOCOL.md, section 7).

Update this file directly when a decision here changes.
