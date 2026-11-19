# Debuggatha

## Purpose

A senior code reviewer, not a chat wrapper. Debuggatha understands a repository's context (its stack, its architecture, its own stated rules) before it reviews anything, and every finding it produces traces back to a real source. Thesis: **"A senior reviewer that understands repository context, applies explicit criteria, and produces traceable findings."** It guides architecture, UX and feature decisions regardless of which model is fashionable.

Debuggatha never writes a user's code. Claude Code, Copilot or Cursor write; Debuggatha reviews. It runs as an MCP server, a CLI and a VS Code extension over one engine. MCP is plumbing, not the pitch: it is a specialist an agent calls, never "an MCP server" or "another AI chat".

## Topology

Polyglot workspace, Rust first ([ARCHITECTURE.md](ARCHITECTURE.md) has the detail):

- `crates/` is the product: `debuggatha-core` (the review domain, depends on nothing), `debuggatha-engine` (runs reviews) and `debuggatha-cli` (the `debuggatha` binary: commands, `serve` for Protocol v1, `mcp`).
- `npm/` holds the two launchers (`@sxnnyside/debuggatha-cli` and `-mcp`), plain JavaScript that runs the binary and nothing else.
- `apps/vscode` is the only TypeScript left: **Node + pnpm** (esbuild, vitest, `vsce`), a client of the binary. Read [apps/vscode/CLAUDE.md](apps/vscode/CLAUDE.md) before touching it.

Contracts are in [docs/schemas/](docs/schemas/README.md), the protocol in [docs/PROTOCOL.md](docs/PROTOCOL.md), the plan and its record in [docs/ROADMAP.md](docs/ROADMAP.md), the decision in ADR-0006.

## Commands

- `just install` — fetch the crates and turn on the git hooks
- `just dev` — run the extension in watch mode
- `just check` — the gate: Rust format, clippy and tests with `cargo deny`; the extension's gate; the npm packages' dry run. Exactly what CI runs; it must pass before any commit or push.
- `just build` · `just test` · `just clean`

`just --list` shows every recipe; prefer `just` over calling `cargo` or `pnpm` directly. Git hooks: pre-commit runs `just check`, commit-msg enforces Conventional Commits.

## Where things go

| What | Where |
|---|---|
| Review domain | `crates/debuggatha-core/src/`: snake_case modules, one folder per subsystem |
| Review execution, detectors, analysis | `crates/debuggatha-engine/src/`; detectors in `detect/rules/` |
| Review Packs, queries, concept tables | `crates/debuggatha-engine/data/` (kebab-case data files; packs in `data/packs`, TOML) |
| CLI commands, MCP server, reports | `crates/debuggatha-cli/src/commands/`, `mcp/`, `report/` |
| npm launchers | `npm/cli`, `npm/mcp`: one `bin.js` each |
| VS Code extension | `apps/vscode/src/`: kebab-case modules |
| Tests | In the file, or `crates/*/tests/<concern>/`; extension: `<name>.test.ts`, `<name>.itest.ts` |
| Scripts | `scripts/`: kebab-case (`.sh`, `.mjs` for npm tooling) |
| Docs, ADRs, schemas | `docs/`, `docs/adr/`, `docs/schemas/`; the glossary is `docs/glossary.md` |

(See `docs/glossary.md` for domain terms.)

## Conventions

### Non-negotiables

- **Never opine without evidence.** Every finding cites at least one of: a rule from the repository itself, a Review Pack, a language or framework convention, a recognized standard (OWASP, WCAG...), the output of an installed analyzer, or evidence in the analyzed code. Never "I think..."; always "This finding exists because...". No source, no finding.
- **The deterministic core does not depend on a model.** Core Skills, detectors, analyzers, the ledger and every finding a review guarantees work with no model at all. A model is an optional extra; it never closes, hides or lowers a finding.
- **A review must say what it could not check.** "0 findings" never stands for "clean" when a language had no parser or a rule had no detector.
- **One fixed identity.** Direct, precise, never condescending; it orients first (Core Skills), states its understanding of the project in one line before any judgment, and never fakes certainty. No personality selector.

### Trust boundaries

- **External analyzers** run as separate processes and are never bundled or depended on. Each has a trust class: `safe` (only reads source), `runs-project-code` (ESLint, PHPStan, Clippy) or `network` (OSV-Scanner). The last two run only when the person enables them (a flag, an environment variable or user-scope settings). A repository file (`.debuggatha/config.json`, workspace settings) or an MCP tool call can never enable them.
- **The semantic layer** is off by default and enabled only by the person running the review. Code goes only to `localhost` (or, over MCP, to the connected client's own model through sampling). Lines that may hold a secret and secret files are never sent. A claim is kept only when it quotes the line it is about and the quote is on that line.
- **Secrets** are redacted in excerpts before they reach reports, the ledger or a model.
- **Git refs** are validated and passed to git without a shell.
- **Errors** reaching an MCP client are sanitized: no raw paths or stack traces.
- **The extension** runs only built-in detectors in an untrusted workspace, and its analyzer and local-model settings are machine scope.
- **A config file can only make a review stricter or quieter**, never enable something that runs code, uses the network or sends code to a model.

### The engine: native Rust (ADR-0006, accepted)

- **The core is language-agnostic by design.** No detector or analysis is written against one language's syntax tree: they read a language-neutral semantic model that per-language adapters fill, and rules name concepts that data tables bind to each language. JavaScript is one adapter among many. JS/TS use `oxc`; other languages use tree-sitter. Packs are TOML, validated against a schema.
- **No regex is added to stand in for a parser.** A rule that needs scope, call structure or data flow waits for the syntax layer; it is not approximated.
- **The old TypeScript engine is frozen.** It is in the git history (tag `ts-engine-final`); the conformance fixtures and the goldens in `corpus/` are what it left, and the Rust tests compare with them. Every intended difference is in [docs/PARITY.md](docs/PARITY.md); a difference not listed there is a bug. The ledger fingerprint stays byte-identical.
- When a rule here conflicts with this direction (for example, the semantic layer over MCP), the trust boundary stays and the mechanism changes; see ADR-0006 and PROTOCOL.md section 7.

### Skill taxonomy

Three tiers; do not flatten them into one list.

- **Core Skills** always run, are deterministic and model-independent; each claim carries an evidence trail, not just a boolean (`core/repository_intelligence`). *Stack Detection* reads manifests and resolves ambiguity by content, not filename alone (a `Cargo.toml` is only "Tauri" if it depends on tauri); the result is an additive `Capability[]`, each with confidence, evidence and origin. *Criteria Resolution* builds a model of the project's criteria from every signal: docs (ARCHITECTURE.md, CONTRIBUTING.md, CLAUDE.md), config surfaces (.eslintrc, rustfmt.toml, biome.json, detekt.yml, .editorconfig) and what manifests imply. *Documentation Context* reads README, ADRs and roadmaps for intent. *Repository Understanding* raises a question only when a concrete gap exists; it never invents an answer.
- **Review Skills** produce judgment per scope (file, diff, architecture) over one shared detector engine that turns a resolved `ReviewPolicy` and source units into citable findings.
- **Analysis Skills** discover, they do not judge: module boundaries, dependency graph, layer model and violations, cycles, public API surface, dead code, ownership and change impact. Architecture Review consumes them.

### Review Packs

A pack is Skills (capabilities it activates), Policies (the assembled, citable ruleset for a review), Rules (atomic, individually citable statements) and Knowledge (the domain expertise the rules derive from). There are stack packs (Rust, Flutter, React, Tauri, Kotlin...) and concern packs (OWASP, Accessibility, Performance, DX, Release...). The spec is [docs/PACK_SPEC.md](docs/PACK_SPEC.md); content lives in `crates/debuggatha-engine/data/packs`.

- A **Review Policy** is a pack's Rules plus the repository's resolved criteria, assembled for one review run. It is always inspectable and citable, and its id is deterministic (ADR-0005).
- Conflicts between rules are recorded with a fixed precedence, never silently dropped (ADR-0003).
- Rules match against the capability set, not exact stack strings.

### Findings Ledger and memory

- The ledger (`.debuggatha/ledger.json`) persists findings across sessions and clients: a stable identity (file + rule id + content anchor, never line number alone), a five-state lifecycle (open, acknowledged, resolved, dismissed, reopened) enforced as a state machine, and an append-only history. Writes are atomic. Findings no longer detected within the reviewed scope resolve; resolved ones that reappear reopen. A finding from a tool or model that did not run is not called fixed.
- Repository Memory (`.debuggatha/memory.json`) keeps accepted deviations, suppressions, exceptions and conventions, with its own lifecycle (detected, suggested, confirmed, active, deprecated, archived). Only `active` items filter findings, and only a person can confirm one. Suppressed findings are reported, never silently hidden.
- The baseline (`.debuggatha/baseline.json`) records what a repository has today so later reviews report only what is new; the ledger still tracks everything.
- The ledger is a persistent record of findings; `RepositoryContextCache` is an in-process cache of the repository snapshot. Different subsystems, different lifetimes.

### Code layout

Modules are folders named for what they hold and are reached through their module file (`foo.rs` with `foo/`; no `mod.rs`). Unit tests are in the file; integration tests are in `tests/`, grouped in a folder per concern. Every clippy `allow` carries its reason.

### Interfaces

- **MCP** (`@sxnnyside/debuggatha-mcp`): review tools (`review_changes`, `review_diff`, `review_files`, `review_workspace`), ledger tools (`list_findings`, `get_finding`, `update_finding`, `explain_finding`, `suppress_finding`, `create_baseline`) and context tools (`repository_context`, `repository_summary`, `list_analyzers`), over stdio and Streamable HTTP. Prompts: `review-flow`, `fix-findings`, `triage-findings`. Every handler delegates to the engine.
- **CLI** (`@sxnnyside/debuggatha-cli`): `review`, `baseline`, `findings`, `memory`, `repository`, `packs`, `policies`, `analyzers`, `init`, `trends`, `doctor`, `completion`. Exit codes: `0` clean, `2` findings at or above the threshold, `1` error.
- **VS Code** (`apps/vscode`): native components only (diagnostics, tree views, status bar, hover, quick fixes). Reviews run in a separate process.

### Model strategy

Default: no model, only deterministic detectors plus installed analyzers. Optional semantic pass: a local runtime (Ollama, LM Studio) or, over MCP, the connected client's own model through sampling. Sampling is deprecated in MCP 2026-07-28, which forbids server-to-client requests; the engine adds an input-required and resume path for those hosts (PROTOCOL.md, section 7).

### Vocabulary

| Don't say | Say | Why |
|---|---|---|
| Prompt | Skill | Not text pasted to a model, but something Debuggatha knows how to do. |
| Templates / Knowledge Packs | Review Pack | Consumed to review, not just to "know." |
| Prompt (per run) | Review Policy | The auditable criteria behind a specific finding. |
| Criteria Loading | Criteria Resolution | Resolving a model from every signal, not loading markdown. |
| Personality (plural) | Identity (singular) | One fixed voice. |

## Do not

- Bring back a personality selector.
- Let a Review Skill run before Core Skills have oriented on the repo.
- Ship a finding with no citable source.
- Make a Core Skill or a guaranteed finding depend on a model provider.
- Let a repository file or a tool call enable an analyzer that runs code or uses the network, or the semantic layer.
- Put business logic in an adapter.
- Market or design around "MCP server" as the pitch.
- Add a regex to stand in for a parser, or regenerate the frozen fixtures and goldens.
- Commit or push without `just check` passing.

Update this file directly when a decision here changes.
