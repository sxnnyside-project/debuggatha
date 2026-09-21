# @debuggatha/engine

Runs reviews. The one package every adapter (`@sxnnyside/debuggatha-mcp`, `@sxnnyside/debuggatha-cli`, `apps/vscode`)
depends on: it orchestrates the review pipeline on top of the domain in `@debuggatha/core` and the
rules in `@debuggatha/packs`, and re-exports the public surface adapters need.

## Modules

Design notes for `skills`, `analysis-engine`, and `policies` are in the [Module reference](#module-reference) below.

| Module | Owns |
| --- | --- |
| `pipeline.ts` | `executeReview` (`analyzeReview` then `concludeReview`), the single orchestration every adapter calls |
| `skills` | The Review Skills (file, diff, architecture) and the rule detectors that turn rules into findings |
| `analysis-engine` | Deterministic repository analysis: dependency graph, cycles, layers, dead code, public API, ownership |
| `analyzers` | External analyzers run as separate processes (gitleaks, Biome, ESLint, Ruff, ktlint, detekt, PHPStan, Clippy, OSV-Scanner), their trust classes, and the license gate metadata |
| `semantic` | The optional model pass: grounded claims only, loopback-only, `executeReviewWithSemantics` |
| `runtime-engine` | The Ollama and LM Studio providers the semantic pass talks to |
| `policies` | Assembles a `ReviewPolicy` from the pack registry and the repository's context |
| `scan-scope` | What counts as the project's own code (no dependencies, build output, generated or ignored files) |
| `infrastructure` | I/O adapters: `LocalGitProvider` |
| `baseline.ts` | The baseline of known findings for adoption mode |
| `provider.ts` | `RepositoryProvider`, the interface for repository access |

## `executeReview`

Build a `RepositoryContext`, request every registered pack (rule resolution keeps only the rules
that fit the detected capabilities, so callers need not name packs; named packs win conflicts),
assemble the `ReviewPolicy`, run the Review Skill for the scope, run the enabled external analyzers,
apply inline suppressions and Repository Memory, then sync the result into
`.debuggatha/ledger.json` (skipped with `persist: false`) and report what the change introduced,
fixed, and reopened. `executeReviewWithSemantics` adds the model pass on top and never lets it close,
hide, or lower a finding.

## Boundaries

- Depends on `@debuggatha/core` and `@debuggatha/packs`; nothing in either depends back on it. Only
  the adapters depend on this package.
- No transport-specific code (MCP schemas, CLI parsing, VS Code API calls).
- No new domain rules or lifecycle logic: those belong in `@debuggatha/core`.
- Analyzers that run project code or use the network, and the semantic pass, are enabled only by
  the options an adapter passes on the person's behalf, never by repository content.

## Limitations

Rule detectors are line-level, code-aware pattern checks, not full AST analysis, and not every pack
rule has one. Architecture Review runs the rule detectors and the analysis engine's layer and cycle
findings; it does not detect which architectural pattern a repository follows.

---

## Module reference

## engine/skills

The Review Skills: one deterministic engine that turns a resolved `ReviewPolicy` plus source units into citable `Finding[]`.

### Architecture

```text
reviewArchitecture(rootDir, policy)  ─┐
reviewDiff(diff, policy)             ─┼─→ SourceUnit[]  (whole tree / added diff lines / named files)
reviewFiles(paths, policy)           ─┘        │
                                               ▼
                                 runReviewSkillsEngine(policy, units)
                                               │  for each policy.rules[i]:
                                               │    detector = detectorFor(rule.id)
                                               │    for each unit: run detector, turn hits into Finding[]
                                               ▼
                                           Finding[]
```

The three public functions differ only in which `SourceUnit[]` they hand the shared engine: the review scope changes, the review engine does not.

- **File Review** reads each named file whole.
- **Diff Review** parses a unified diff and hands the engine only the added lines, each mapped back to its real line number, so a diff review never becomes a workspace sweep.
- **Architecture Review** walks the whole tree (skipping dependencies, build output, and ignored files) and turns `engine/analysis-engine`'s layer violations and dependency cycles into findings (`buildAnalysisFindings`), citing `framework-convention` evidence rather than a pack rule. It does not re-walk the dependency graph itself.

`runReviewSkillsEngine` is the one place a `Finding` is built: every hit cites `code` evidence (the exact excerpt and line) plus `review-pack-rule` (pack rules) or `criteria` (repository-derived rules) evidence, and carries `appliedPolicy` and `originatingPack`. "Never opine without evidence" is a runtime property of every finding.

### Detector registry

A `Rule` is declarative data; the judgment "this code violates it" belongs to a Skill. `RULE_DETECTORS` is that judgment: a hand-written `Record<ruleId, RuleDetector>` of code-aware line scans (strings and comments do not match), not a pattern inferred from a rule's prose. Coverage is intentionally partial: a resolved rule with no detector is visible in the policy but produces no findings. Rules that need type information or structural analysis to avoid unacceptable false positives (`no-floating-promises`, `consistent-type-imports`, React's `rules-of-hooks`, Go's `explicit-error-check`...) are left undetected rather than approximated with a noisy regex.

### Technical decisions

- **Deterministic judgment.** The detector registry works with no model. A model is an optional extra handled by `engine/semantic`, which never closes, hides, or lowers a finding.
- **`SourceUnit.lineNumbers`** lets Diff Review map a hit in "just the added lines" to the real line; `undefined` means the content is the whole file.
- **Layer and cycle findings need no pack.** The Analysis Engine's dependency graph is their evidence, so they cite `framework-convention` and carry no `appliedPolicy` or `originatingPack`.
- **Exact edits.** A few detectors (`no-var`, `eqeqeq`, `no-explicit-any`, `avoid-print`) attach the line, columns, and replacement, marked `safe` or `review`.

### Limitations

- Detector coverage is partial: a cross-section of the TypeScript, JavaScript, Dart, and release packs, plus a few rules from React, Vue, Rust, and Go.
- Architecture Review does not detect which architectural pattern a repository follows; it runs rule detectors across the tree plus layer and cycle findings.
- There is no AST or type-aware analysis: every detector is a line-level scan, so it cannot see across statements.

---

## engine/analysis-engine

Deterministic, static repository analysis that enriches reviews instead of existing as isolated
utilities. No model is involved anywhere in this module.

### Architecture

```text
inferModuleBoundaries(rootDir)
        │  prefers packages/*, apps/* (a real workspace layout);
        │  falls back to src/<name>/* directory-name heuristics only
        │  for a single-package repo with no workspace layout to read
        ▼
ModuleBoundary[]
        │
        ▼
buildDependencyGraph(rootDir, boundaries)
        │  declared edges from each boundary's own package.json;
        │  import edges from scanning each boundary's own source files
        ▼
DependencyGraph { nodes, edges }
        │                              ┌─→ detectCycles(graph)               → DependencyCycle[]
        ▼                              │
buildLayerModel(boundaries) ───────────┤
        │                              └─→ detectLayerViolations(graph, layerModel) → LayerViolation[]
        ▼
LayerModel { assignments }

boundaries ──→ buildPublicApiSurface(rootDir, boundary)  → PublicApiSurface  ──┐
                                                                                 ├─→ detectDeadCode(rootDir, boundaries, publicApis) → DeadCodeFinding[]
boundaries ──→ detectOwnership(rootDir, boundaries)      → OwnershipSignal[]  ─┘

(changedFiles, boundaries, graph, publicApis) ──→ estimateChangeImpact(...) → ChangeImpact
```

`runAnalysis(rootDir, { cache? })` (`pipeline.ts`) composes every stage
above except Change Impact — that one is a separate, on-demand entry
point (`estimateChangeImpact`) since it needs a set of changed files as
input, not something derivable while analyzing the repository at rest.
Every stage consumes the previous stage's output rather than
rediscovering it — Layer Violations doesn't re-walk imports, Dead Code
doesn't re-scan for exports, Change Impact doesn't rebuild the graph.

### Cache strategy

`AnalysisCache` (in-memory, injectable, like `RepositoryContextCache`: no hidden global
singleton) keyed by `rootDir`.
Invalidation is a fingerprint of every directory's listing (catches a
file being added or removed) plus every file's mtime/size (catches a
tracked file's content changing) — `buildAnalysisFingerprint`/
`fingerprintChanged` in `cache.ts`, deliberately not shared with Repository
Intelligence's own fingerprint implementation (a distinct data shape:
source files across the whole repo for graph-building, not the manifest/
doc signals `RepositoryContext` tracks) even though the *strategy* mirrors
it.

### Performance considerations

- Every stage is a single filesystem walk plus regex/line scans — no
  parsing into an AST, no external process. The most expensive
  repeated cost is `buildDependencyGraph` re-reading every source file's
  content to scan import specifiers; this is why the cache exists and
  why the pipeline runs it once per `rootDir`, not once per Review Skill
  invocation.
- `walkSourceFiles`/directory walks skip `node_modules`, `dist`, `build`,
  `.turbo`, `coverage`, `.next`, `.venv`, and any dotfile/dotdir — the
  same ignore list the Review Skills' file walking uses.

### Known limitations

- **Module Boundaries' single-package fallback is a directory-name
  heuristic** (`src/domain`, `src/adapters`, etc.) — it does not assume
  one architectural style, but a repo using unconventional directory
  names produces boundaries of kind `"domain"` (the least presumptive
  default) rather than nothing.
- **Layer Model assignment is name-based, not structural** — a module
  named `packages/domain-users` is assigned layer `"domain"` purely from
  its id/name matching `LAYER_VOCABULARY`; a module violating its own
  apparent naming convention in practice would be mis-assigned. Modules
  matching no vocabulary are `"unknown"`, never guessed.
- **Dead Code is name-based, not scope-aware.** `detectDeadCode` counts
  bare-identifier occurrences repository-wide — a symbol whose name
  happens to collide with something in an unrelated file (e.g. two
  modules both exporting a function called `run`) suppresses a real dead
  export as a false negative. This is intentional: a false positive is
  worse than a false negative, so this stage only
  ever reports `"exported-but-unused"`/`"probably-dead"` at `medium`/`low`
  confidence and never claims `"unreachable"`/`"unreferenced"` (both
  `DeadCodeKind` values exist in `types.ts` for future use but nothing
  currently produces them — real control-flow analysis is required first).
- **Public API Surface's entrypoint detection is manifest+convention
  based**, not bundler-aware — it does not know about package.json
  `exports` subpath maps, so a module with multiple legitimate public
  entrypoints (subpath exports) will have some of them misclassified as
  `unexpectedlyExposed`.
- **Ownership Detection only reads `CODEOWNERS`.** Repository-convention
  inference (e.g. "whoever touches this directory most") is deliberately
  not implemented — `OwnershipSource` has a `"convention"` value reserved
  for it, but every module without a matching `CODEOWNERS` rule reports
  `"unknown"` rather than a guessed heuristic owner.
- **No model anywhere in this module**, by design: every stage is static analysis. Consumers
  (Review Skills) consume these results; this module never calls out to a model.

### Integration

Architecture Review (`engine/skills`) consumes `runAnalysis`'s layer violations and cycles through
`skills/engine/analysis-findings.ts#buildAnalysisFindings`, so the dependency graph is walked once.

### Not built

- Wiring Public API Surface against `package.json` `exports` subpath maps
  to stop misclassifying legitimate subpath entrypoints.
- A structural/AST-aware Dead Code pass (to safely add
  `"unreachable"`/`"unreferenced"` detection).
- CODEOWNERS-absent repos: a conservative, evidence-cited
  `"convention"` ownership source (e.g. "every file in this directory was
  last touched by the same author" from git blame) instead of leaving
  every module `"unknown"`.
- Feeding `ChangeImpact` into Diff Review so a diff review can
  report "this change's blast radius" alongside its line-level findings.

---

## engine/policies

Wiring between the Knowledge System's policy resolution and the concrete Review Pack catalog. `core/knowledge-system`'s `resolvePolicy` accepts any populated `CapabilityRegistry`; this module populates one from `@debuggatha/packs` and gives callers a single entry point.

- `defaultCapabilityRegistry`: the one populated registry (ADR-0004), built once at module load.
- `assemblePolicy(context, request, registry?)`: calls `resolvePolicy` against that registry or a caller-supplied one.

No resolution algorithm lives here (that is `core/knowledge-system`) and no pack content (that is `@debuggatha/packs`). It depends on `core/knowledge-system` and `@debuggatha/packs`, never on an adapter.

**Limitation:** no `SkillDescriptor` catalog is registered, so a pack declaring a `{ skillId }` dependency fails closed.
