# Architecture

Debuggatha is a Bun + Turborepo monorepo. The engine is distributed as an MCP server first; a VS Code panel is a client of that engine, built last, not first.

## Structure

```text
debuggatha/
├── apps/
│   ├── vscode/          — VS Code client of @debuggatha/core
│   └── docs/             — docs site (not yet implemented)
│
├── packages/
│   ├── repository-intelligence/ — Epic 1: RepositoryContext (stack, deps, docs, criteria)
│   ├── review-engine/     — Epic 2: review domain (Request/Session/Finding/Result)
│   ├── knowledge-system/ — Epic 3: Skills/Packs/Policies domain (Registry, resolvePolicy, assembleContext)
│   ├── findings-ledger/   — Epic 4: persistent history of findings (lifecycle, matching, sync)
│   ├── skills/             — Epic 11: Review / Analysis skill implementations (Core Skills moved to repository-intelligence)
│   ├── analysis-engine/  — Epic 12: deterministic module boundaries/dependency graph/dead code/ownership pipeline
│   ├── context-intelligence/ — Epic 14: documentation intent, workflow, ownership, metadata, git context (ContextItem[])
│   ├── repository-memory/ — Epic 15: long-term engineering knowledge (suppressions, deviations, conventions, decisions)
│   ├── runtime-engine/   — Epic 13: provider-agnostic semantic review execution (Ollama, LM Studio)
│   ├── review-packs/    — Review Pack content conforming to knowledge-system's schema
│   ├── policies/          — thin wiring: calls knowledge-system's resolvePolicy against a registry populated from review-packs
│   ├── core/              — façade: re-exports every domain package; orchestrates them together (executeReview)
│   ├── infrastructure/   — concrete RepositoryProvider implementations (LocalGitProvider)
│   ├── mcp/                — MCP server (distribution layer only)
│   ├── cli/                 — standalone CLI, for CI usage
│   └── testing/           — test fixtures shared across packages
│
├── turbo.json
└── bun.lock
```

Cross-package imports use short aliases (`@core`, `@skills`, `@packs`, `@policies`, `@mcp`, `@cli`, `@shared`, `@repo-intel`, `@review-engine`, `@knowledge-system`, `@ledger`) configured in `tsconfig.base.json`, backed by real workspace package names (`@debuggatha/*`) for Bun's module resolution.

## Subsystems Overview

**Repository Intelligence** (`@debuggatha/repository-intelligence`): Stack Detection, Dependency Context, Documentation Context, Criteria Resolution, Repository Understanding, the immutable `RepositoryContext` snapshot, and its fingerprint-based cache.

**Review Engine domain** (`@debuggatha/review-engine`): the review domain model — `ReviewRequest`, `ReviewSession` with an enforced lifecycle, `Finding` (can't be constructed without evidence and a location), `Evidence`, `Category`, `Severity`/`Confidence`, `Recommendation`, `ReviewResult`, `ReviewSummary`.

**Knowledge System** (`@debuggatha/knowledge-system`): the Skill/Review Pack/Review Policy domain — `SkillDescriptor`, `ReviewPack`/ `Rule`/`KnowledgeEntry`, an injectable `CapabilityRegistry`, `resolvePolicy` (Rule Resolution, pure, fail-closed on missing dependencies), `assembleContext` (→ `SkillContext`), `ReviewPolicy`/ `ConflictRecord` (conflicts recorded with fixed precedence, never silently dropped), and `validateReviewPack`. `@debuggatha/review-packs` ships the real `ReviewPack` type plus example packs; `@debuggatha/policies`' `assemblePolicy` really calls `resolvePolicy`.

**Findings Ledger** (`@debuggatha/findings-ledger`): the persistent history of every finding across review sessions — `Ledger`/`LedgerEntry` (immutable), a five-state lifecycle (open/acknowledged/resolved/dismissed/reopened, enforced), append-only `HistoryEvent[]`, Finding Identity via `computeFindingFingerprint` (file + rule id + content anchor, never line number alone), a pluggable `FindingMatcher`, `synchronizeReviewResult` (new/matched/auto-resolved/reopened, scoped to what a review actually covered), `summarizeLedger`, and deterministic versioned persistence at `.debuggatha/ledger.json`.

**Core** (`@debuggatha/core`) re-exports all four as the stable façade other packages depend on.
