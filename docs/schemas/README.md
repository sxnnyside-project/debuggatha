# Contract schemas

<!-- The JSON Schemas of the data Debuggatha stores and exchanges; for adapter authors. -->

JSON Schema (draft-07) for the data Debuggatha stores and exchanges. They are the target the Rust engine must reproduce (ADR-0006) and the shapes [PROTOCOL.md](../PROTOCOL.md) refers to.

---

| File | Describes | Where it appears today |
|---|---|---|
| `finding.schema.json` | One finding with locations, evidence and recommendations | Review results, MCP payloads; **generated from Rust** |
| `review-result.schema.json` | The result of a review (summary, findings, recommendations, execution) | `reviewResult` in the CLI JSON report |
| `ledger.schema.json` | `.debuggatha/ledger.json` | Persisted; **generated from Rust** |
| `memory-store.schema.json` | `.debuggatha/memory.json` | Persisted |
| `baseline.schema.json` | `.debuggatha/baseline.json` | Persisted |
| `repository-context.schema.json` | The normalized description of a repository (stack, capabilities, dependencies, documentation, criteria, understanding) | Engine output, input of policy resolution; **generated from Rust** |
| `review-request.schema.json`, `review-session.schema.json` | One requested review, and the record of one in progress (lifecycle status, selected policy and packs) | Engine-internal; **generated from Rust** |
| `review-scope.schema.json` | What a review covers (workspace, diff, files, changes) | Engine input |
| `review-changes.schema.json` | Introduced, fixed and reopened findings of a run | Engine output |
| `analyzer-options.schema.json`, `analyzer-run.schema.json` | Which analyzers may run, and what happened | Engine input and output |
| `semantic-run.schema.json` | What the semantic pass did | Engine output |
| `review-job.schema.json`, `review-job-result.schema.json` | The shape of a review request and its answer; `apps/vscode/src/contracts` is generated from `ledger`, `repository-context` and `review-job-result` | `apps/vscode/src/daemon/api.ts` |

## How they are made

The files marked "generated from Rust" (`finding`, `ledger`, `repository-context`, `review-request`, `review-session`) are written by `generated_schema` from the Rust types, and a test fails when they are stale (`DEBUGGATHA_UPDATE_SCHEMAS=1 cargo test -p debuggatha-core contract` rewrites them). `generated_schema` closes objects, drops `null` from optional fields (absent is how a file says "none") and strips `uint32` noise. The rest are reviewed by hand and checked against the Rust types by tests: `memory-store` (a stricter union of six kinds than the Rust item), `review-result` (needs `Coverage`, an engine type), and `baseline`, `review-changes`, `analyzer-*`, `semantic-run`, `review-job*` and `review-scope`.

Conventions the hand-reviewed schemas keep: counters, durations, lines and columns are `integer` with a minimum (0 for counts and durations, 1 for lines, columns, `timeoutSeconds` and `schemaVersion`); timestamps (`*At`) carry `format: date-time`; `Finding.evidence` and `Finding.locations` have `minItems: 1`, because a finding must point at something and be backed by evidence. `Category` accepts any string, since packs are data and may add categories.

## Coverage

`Coverage` (in `review-result` and `review-job-result`) is how a review states what it could not check:

- `complete`: derived by the engine, true only when every language is fully parsed, no rule has a gap (except `disabled-by-config`, the person's own choice) and no file was skipped for a reason that hides code (`too-large`, `unreadable`, `not-utf8`; `ignored`, `binary`, `generated`, `symlink-outside-root` are scope decisions).
- `languages`: per language, files, files with errors, status (`parsed`, `parsed-with-errors`, `parsed-limited` for partial grammars such as C++, `not-parsed`) and parser. `fileDetails` (one entry per file) is reserved for a `coverageDetail: "files"` option the daemon does not take yet; the report carries the summary, so it does not grow with the repository.
- `checkedRules`: rules evaluated, per pack and version. With `ruleGaps` it accounts for every rule of the active packs.
- `ruleGaps`: `no-detector`, `needs-type-information`, `needs-repository-files`, `language-not-parsed`, `analyzer-not-installed`, `analyzer-not-enabled`, `analyzer-failed`, `timeout`, `disabled-by-config`.
- `skippedFiles`: by reason (`ignored`, `binary`, `generated`, `symlink-outside-root`, `too-large`, `unreadable`, `not-utf8`).

---

*Last updated: 2026-11-19*
