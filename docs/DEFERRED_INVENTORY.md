# Deferred Inventory

Produced by Epic 16A (research-only audit), resolved by Epic 16B
(this document reflects the post-resolution state). This is the living
record of intentionally-deferred work — items here are deferred on
purpose, not forgotten.

## Resolved in Epic 16B

- **A — Stale documentation from pre-Epic-11 packages.** Corrected in
  place (with a strikethrough + resolution note, not deleted) in
  `packages/review-engine/README.md`, `packages/knowledge-system/README.md`,
  `packages/policies/src/index.ts`, `packages/findings-ledger/README.md`.
- **B — CLAUDE.md's stale CLI claim.** `review --files` has worked since
  Epic 11; the claim it didn't is removed. CLAUDE.md's Epic 6 entry
  updated to reflect H's fixes below.
- **G — Missing detector coverage (partial, by design).** Added five new
  detectors: `no-dangerously-set-inner-html` (react),
  `use-v-html-carefully` (vue), `no-unwrap-expect`/`no-unsafe-blocks`
  (rust), and a shared `no-panic` (rust + go, disambiguated by file
  extension). Deliberately modest — see "Intentionally still deferred"
  below for what remains.
- **H — CLI gaps.** `policies` command now assembles and displays the
  real `ReviewPolicy` for the current repository (via
  `defaultCapabilityRegistry`/`assemblePolicy`) instead of a hardcoded
  entry. `review` now exits `0` (no findings), `2` (findings detected),
  or `1` (the review itself errored) instead of always `0`/`1`.
- **N — No Repository Memory adapter surface.** Added a full `debuggatha
  memory` CLI command group: `list`, `suggest`, `confirm`, `activate`,
  `reject`, `deprecate`, `archive`, `remove` — thin wiring over
  `@debuggatha/repository-memory`'s existing, already-tested API.
- **K — `packages/review-packs` test coverage.** Added catalog-wide
  tests: no duplicate pack ids, every pack has a non-empty
  `displayName` and at least one rule, every rule statement is
  non-placeholder prose. (`validateReviewPack` already covered all
  *within*-pack invariants; these are the catalog-wide checks it
  structurally can't do.)
- **Bug (discovered during N's implementation, not in the original
  inventory) — `command.parent?.logger` was `undefined` in every nested
  CLI subcommand.** `packages/cli/src/commands/findings.ts` (pre-existing,
  affecting `findings list`/`show`/`resolve`/`dismiss`/`reopen`/`summary`)
  used the wrong command reference to reach the logger the global
  `preAction` hook attaches — Commander passes the actual executing
  (leaf) command as the action's last argument, not that command's
  parent. This meant **every nested `findings` subcommand crashed on
  invocation** (verified via direct reproduction: `debuggatha findings
  list` threw `Cannot read properties of undefined (reading 'error')`
  before this fix). Fixed to `command.logger` in both `findings.ts` and
  the new `memory.ts` (which copied the same, then-still-buggy, pattern).
  Verified via a full manual CLI lifecycle test: create → suggest →
  confirm → activate → re-run review → finding auto-suppressed, exit
  code `0`.
- **C — `apps/docs` placeholder.** Added `apps/docs/turbo.json`
  (`{"extends": ["//"], "tasks": {"build": {"outputs": []}}}`) so
  `turbo run build` no longer warns about a missing `dist/**`. Added
  `apps/docs/README.md` stating plainly that this is an intentional
  placeholder, not abandoned or forgotten work.
- **S — Turbo build warning.** Resolved as a side effect of C.

## Corrected during resolution (verify-before-implement)

- **I — "VS Code git provider bypasses `RepositoryProvider`."** On
  closer inspection, `apps/vscode/src/providers/gitProvider.ts`'s
  `VSCodeGitProvider` **already implements `RepositoryProvider`**
  (`getRootDir`/`getDiff`/`getStatus`/`getBranch`) — the same interface
  `packages/infrastructure`'s `LocalGitProvider` implements for the CLI.
  Both implementations shell out to `git` via `execSync` internally,
  which is expected and correct — that's what implementing the
  interface *means*, not a bypass of it. The original Epic 16A item
  overstated the gap (likely conflating "still uses `execSync`
  internally" with "doesn't implement the abstraction"). The one real,
  much smaller remaining inconsistency — `VSCodeGitProvider` takes `cwd`
  as a per-call parameter on some methods instead of binding it at
  construction the way `LocalGitProvider` does, and exposes two extra
  methods (`getUncommittedFiles`, `readFileAtCommit`) beyond the shared
  interface — is cosmetic, not architectural, and not worth a refactor
  on its own. **Downgraded from P2 architecture item to informational.**

## Intentionally still deferred

Each item below was evaluated during Epic 16B and deliberately left
alone — rationale given per the resolution rules (architectural
redesign, new subsystem, research, or explicit constraint conflict).

- **E — YAML/TOML parser dependency decision.** Requires a repo-wide
  dependency decision (which parser, bundle-size impact across every
  consuming package) — genuinely a research/design decision, not a
  quick fix. Still affects: `.eslintrc`/`.prettierrc` YAML variants
  (`packages/repository-intelligence`), `detekt.yml` content
  (`packages/repository-intelligence`), GitHub Actions workflow parsing
  beyond `on:` triggers (`packages/context-intelligence`).
- **F — AST/heuristic detection ceiling.** Both `packages/skills` and
  `packages/analysis-engine` hit the same structural ceiling
  (line/name-based heuristics, no real AST). A shared AST layer is a
  larger architectural undertaking explicitly out of scope for a
  stabilization pass.
- **J — Thin Foundation Bundle packs** (`hono`, `elysia`, `owasp`,
  `accessibility`, `performance`, `architecture`, `dx` — 3-4 rules
  each). Expanding pack *content* is explicitly excluded by this epic's
  constraint ("Do not expand the Foundation Bundle").
- **L — `apps/vscode` mocked-domain adapter tests.** Would need new
  test infrastructure for mocking the `vscode` module (no equivalent to
  MCP's `DomainDeps` injection pattern exists for the extension's
  command layer yet) — a real testing-infrastructure investment, not a
  small addition.
- **M — VS Code "Review Selection" whole-file fallback.** Implementing
  real selection-scoped review is a product feature (a new review
  scope kind, line-range-aware skill execution) — out of scope
  ("do not introduce new features").
- **O — No automatic `detected` → `suggested` promotion for Repository
  Memory.** Needs a real detector/producer (what triggers a
  suggestion?) — a new feature, not adapter wiring.
- **P — No recurring-finding detection.** Explicitly Research category;
  needs a design decision on what "recurring" means (same finding N
  times? per rule? per file?) before any implementation makes sense.
- **Q — ASP.NET Core (`.csproj`)/PHP-via-Composer stack detection
  gaps.** Adding new file-type scanning to `scanStack` is new capability
  surface — borderline scope expansion, deferred alongside J.
- **R — MCP sampling never wired (the documented "default" model
  path).** Originally filed **P1** in the inventory. Re-evaluated
  during resolution: implementing it requires (a) a new transport-level
  capability in `packages/mcp` (the `sampling/createMessage` protocol)
  and (b) effectively a new `Provider` implementation backed by MCP
  sampling rather than HTTP — which directly conflicts with this
  epic's explicit constraints **"Do not redesign major subsystems"**
  and **"Do not add new Runtime providers."** Per the Phase 4 rule
  ("if an item requires architectural redesign... do not implement;
  verify the rationale and priority... leave intentionally deferred"),
  this is deliberately **not implemented despite its P1 label** — the
  global resolution constraints take precedence over the phase
  ordering when the two conflict. This is the single largest
  discrepancy between "documented priority" and "what shipped" in this
  resolution pass, and should be the first item on the next roadmap.

## Priority summary (post-resolution)

| Priority | Resolved | Deferred (with justification) |
|---|---|---|
| P0 | — | none identified |
| P1 | A, G (partial), N, H | R (constraint conflict — see above) |
| P2 | H, K | E, F, J, L, M, O, P, Q |
| P3 | C, S | — |
| Corrected (not a real gap) | — | I |

## Validation performed after every change in this resolution pass

`bun install` → `turbo run build typecheck test lint --force` across
the full monorepo, plus targeted manual smoke tests: CLI `policies`
against a real repository, CLI `review` exit codes (0 vs 2) against a
real fixture with an actual finding, and a full Repository Memory
lifecycle end-to-end (`memory suggest` → `confirm` → `activate` → a
second `review` run confirming the finding is now suppressed and the
ledger auto-resolves the old entry). All green throughout.
