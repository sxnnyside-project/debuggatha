# @debuggatha/context-intelligence

Epic 14. Collects, normalizes, and exposes engineering context beyond
what code alone tells you — documentation intent, development workflow,
ownership, project metadata, and git context. See
[CLAUDE.md](../../CLAUDE.md) at the repo root.

**It does not produce findings.** Its responsibility ends at producing
structured `ContextItem[]` — what a Review Skill or the Runtime Engine
does with that context is entirely downstream of this package.

## Architecture overview

```text
Repository
    │
    ▼
buildRepositoryContext (@debuggatha/repository-intelligence, Epic 1)
    │  → RepositoryContext { stack, capabilities, documentation, criteria, ... }
    ▼
buildContextIntelligence(rootDir, repositoryContext, { cache? })
    │
    ├── buildConventionItems(repositoryContext.criteria)         → EngineeringConventionItem[]
    ├── buildDocumentationItems(repositoryContext.documentation) → DocumentationContextItem[]
    ├── buildCapabilityItems(repositoryContext.capabilities)     → CapabilityContextItem[]
    ├── detectWorkflow(rootDir)                                  → WorkflowContextItem[]   (new)
    ├── buildOwnershipItems(rootDir)                             → OwnershipContextItem[]  (via @debuggatha/analysis-engine)
    ├── buildProjectMetadataItems(rootDir)                       → ProjectMetadataItem[]   (new)
    └── buildGitContextItems(rootDir)                            → GitContextItem[]        (new)
    ▼
ContextIntelligenceResult { rootDir, generatedAt, items: ContextItem[] }
```

Three of the seven sources (`buildConventionItems`, `buildDocumentationItems`,
`buildCapabilityItems`) never touch the filesystem at all — they retype
data Repository Intelligence (Epic 1) and Capability Resolution
(Epic 12.5) already parsed, into this package's unified taxonomy.
`buildOwnershipItems` reuses `@debuggatha/analysis-engine`'s
`inferModuleBoundaries`/`detectOwnership` (Epic 12) directly rather than
reimplementing CODEOWNERS parsing. Only `detectWorkflow`,
`buildProjectMetadataItems`, and `buildGitContextItems` read anything new
— "avoid duplicate parsing across subsystems."

## Facts, not interpretation

The single design decision this package is built around: a `ContextItem`
never stores prose or an interpreted conclusion. A `CONTRIBUTING.md`
heading "## Prefer small functions" becomes:

```json
{
  "category": "engineering-convention",
  "rule": "Prefer small functions",
  "source": "CONTRIBUTING.md",
  "confidence": "documented",
  "evidence": [{ "file": "CONTRIBUTING.md", "detail": "heading" }]
}
```

never `"the project favors small functions, so watch for large ones"` —
that judgment (does *this* function violate that rule? how severely?)
belongs to a Review Skill or the Runtime Engine, consuming the fact
later. This is what keeps context reusable, auditable, and safe to
extend: adding a new source never requires deciding what any fact
*means* for a finding, only what the fact *is*.

## Context model

Seven categories (`ContextCategory` in `types.ts`), each its own
interface: `engineering-convention`, `documentation`, `capability`,
`workflow`, `ownership`, `project-metadata`, `git`. Every `ContextItem`,
regardless of category, carries `id`, `confidence`, `evidence: ContextEvidence[]`,
and `source` — the four fields the epic requires on every item.

## Confidence model

Three levels, never conflated:

- **`documented`** — a human wrote this down (a CONTRIBUTING.md heading,
  a CODEOWNERS entry, a README's existence). The strongest claim this
  package makes.
- **`detected`** — a deterministic scan found concrete evidence (a config
  file's presence, a manifest field, a high-confidence `Capability`)
  without anyone writing prose about it.
- **`inferred`** — derived from indirect signals (e.g. a `Capability`
  detected only from a dependency declaration, not a dedicated config
  file — mapped from that capability's own `"medium"`/`"low"`
  confidence).

Never promoted upward: `buildCapabilityItems`'s mapping
(`capabilities.ts#toContextConfidence`) is the clearest example — a
`Capability` is never `"documented"` at all, since nobody hand-writes
"this repo uses TypeScript" for Repository Intelligence to read.

## Evidence model

Every `ContextItem.evidence` entry is `{ file, detail }` — a real path
and a concrete reason, never an opaque "trust me." Items with no
supporting evidence are never emitted at all (see `buildOwnershipItems`:
a module `detectOwnership` couldn't attribute — `source: "unknown"` — is
simply absent from the result, not reported with an empty or guessed
owner list).

## Cache strategy

Targeted, not a full repository walk (unlike Epic 12's Analysis Engine,
which genuinely needs one for its dependency graph). `buildContextFingerprint`
tracks: the input `RepositoryContext.generatedAt` (a cheap proxy for
"has anything Repository Intelligence covers changed"), a small fixed
list of specific files this package reads beyond that
(`package.json`, `composer.json`, license/changelog/release-config/
commitlint files, `.git/HEAD`, `.git/refs/remotes/origin/HEAD`), and
three directory listings (`.` — the repo root, to catch a new root-level
file appearing; `.github/workflows`; `.git/refs/heads`). Invalidates on
any mismatch — "invalidate only affected sources when repository content
changes" in spirit, though the granularity here is "any tracked file
changed" rather than per-source invalidation, since seven cheap sources
don't yet justify seven separate cache entries.

## Integration

Exposed through `@debuggatha/core`'s façade like every other epic's
package. `conventionsToPolicyStatements` is a light, optional integration
seam: it turns `engineering-convention` items back into plain strings —
exactly the shape `@debuggatha/runtime-engine`'s `policyStatements`
parameter (Epic 13) already expects — so a caller can feed repository
conventions into a semantic review's prompt without this package needing
to know anything about the Runtime Engine.

## Testing

27 tests across every source (documentation parsing, repository
metadata, workflow detection — CI trigger extraction, release/commit/
branching convention markers — ownership extraction, confidence
assignment for both `documented` and `inferred` paths, evidence
generation) plus the composed pipeline (cache hit, cache invalidation on
a new root-level file, cache invalidation when the underlying
`RepositoryContext` changes) — using real temp-directory repository
layouts (`@debuggatha/testing`'s `withTempRepo`) and, for git context, a
directly-constructed `.git/HEAD`/`refs` structure rather than a real git
init.

## Remaining future context opportunities

- **Per-source cache invalidation** — today one fingerprint mismatch
  invalidates the whole result; splitting by source would avoid
  recomputing, say, ownership when only a workflow file changed.
- **Commit convention detection from actual commit messages** —
  explicitly deferred by the epic ("do not perform semantic analysis of
  commit history yet"); `commitlint.config.*` presence is a proxy, not a
  measurement of whether commits actually comply.
- **Repository Memory** (named in the epic's "Integration" section as a
  future consumer) — no such subsystem exists yet; this package's
  `ContextItem` model is designed to be that subsystem's input without
  changes when it does.
- **ADR-specific extraction** — ADRs are currently just one more
  `DocumentationContextItem` (kind-tagged, not content-parsed); a
  dedicated ADR status/decision extractor (accepted/superseded/proposed)
  would be a natural, narrow follow-on source.
