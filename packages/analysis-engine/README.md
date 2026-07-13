# @debuggatha/analysis-engine

Epic 12. The Analysis Engine — deterministic, static repository analysis
that enriches every future review instead of existing as isolated
utilities. See [CLAUDE.md](../../CLAUDE.md) at the repo root.

## Architecture summary

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

## Cache strategy

`AnalysisCache` (in-memory, injectable — same discipline as Epic 1's
`RepositoryContextCache`: no hidden global singleton) keyed by `rootDir`.
Invalidation is a fingerprint of every directory's listing (catches a
file being added or removed) plus every file's mtime/size (catches a
tracked file's content changing) — `buildAnalysisFingerprint`/
`fingerprintChanged` in `cache.ts`, deliberately not shared with Repository
Intelligence's own fingerprint implementation (a distinct data shape:
source files across the whole repo for graph-building, not the manifest/
doc signals `RepositoryContext` tracks) even though the *strategy* mirrors
it.

## Performance considerations

- Every stage is a single filesystem walk plus regex/line scans — no
  parsing into an AST, no external process. The most expensive
  repeated cost is `buildDependencyGraph` re-reading every source file's
  content to scan import specifiers; this is why the cache exists and
  why the pipeline runs it once per `rootDir`, not once per Review Skill
  invocation.
- `walkSourceFiles`/directory walks skip `node_modules`, `dist`, `build`,
  `.turbo`, `coverage`, `.next`, `.venv`, and any dotfile/dotdir — the
  same ignore list Epic 11's file-walking uses.

## Known limitations

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
  export as a false negative. This is intentional: the epic requires
  false positives to be worse than false negatives, so this stage only
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
- **No LLM anywhere in this package**, by design (Epic 12 "Deterministic
  First") — every stage above is static analysis. Consumers (Review
  Skills, an LLM-backed judgment layer) consume these results; this
  package never calls out to one itself.

## Integration

Exposed through `@debuggatha/core`'s façade (`packages/core/src/index.ts`)
alongside every other epic's domain package. `@debuggatha/skills`'
Architecture Review (Epic 11) consumes `runAnalysis`'s layer violations
and cycles via `engine/analysis-findings.ts#buildAnalysisFindings`
instead of its own dependency traversal — Epic 11 originally shipped a
bespoke `detectDependencyDirectionViolations` doing exactly this graph
walk; that function is now deleted in favor of this package, per Epic
12's "avoid duplicated graph traversals".

## Future analysis opportunities

- Wiring Public API Surface against `package.json` `exports` subpath maps
  to stop misclassifying legitimate subpath entrypoints.
- A structural/AST-aware Dead Code pass (to safely add
  `"unreachable"`/`"unreferenced"` detection).
- CODEOWNERS-absent repos: a conservative, evidence-cited
  `"convention"` ownership source (e.g. "every file in this directory was
  last touched by the same author" from git blame) instead of leaving
  every module `"unknown"`.
- Feeding `ChangeImpact` into Diff Review (Epic 11) so a diff review can
  report "this change's blast radius" alongside its line-level findings.
