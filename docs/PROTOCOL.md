# Debuggatha Protocol v1

<!-- Protocol v1, the wire contract between the binary and its clients; for adapter authors. -->

How a client (VS Code, the MCP server, a future JetBrains plugin, CI tooling) talks to the engine when the engine is the `debuggatha` binary (ADR-0006). This document is the contract the Rust daemon and every client implement; the data shapes are in [schemas/](schemas/README.md).

---

Status: `debuggatha serve` implements the transport, the handshake and these methods: `hello`, `shutdown`, `review/run` (scopes `workspace`, `files` and `diff`, given as a base or as the diff text), `findings/list` (filters `status`, `severity`, `category`, `file`), `findings/get`, `findings/update`, `findings/explain`, `findings/suppress`, `findings/history`, `baseline/create`, `repository/context`, `repository/summary`, `repository/policy`, `analyzers/list`, `memory/list` and `memory/transition`. Read methods run side by side and answer by id; methods that write take turns. What the person enabled comes from the flags of `serve` (`--analyzer <id>`, `--semantic`), and a request can only narrow it. Cancellation is built: `review/cancel` and the `cancel` event stop the review between steps, kill the analyzer that is running, and the original request then answers `cancelled`; the ledger sync, the last step, either completes or never starts; ending the session cancels what still runs. The `progress` event is sent to a client that asked for it at the handshake (`capabilities.progress`), at most four a second, for the stages `orient`, `detect` (rule by rule), `analyzers`, `semantic` and `sync`. Not yet built: the `log` event, `coverageDetail`, and the schemas for the shapes above. It is derived from what exists today: the former VS Code worker protocol, the MCP tool set (`packages/mcp/src/tools`), and the CLI. The client's own model is built, both as the request `model/complete` and as `input_required` with `review/resume` (`serve --semantic host`).

## 1. Transport

- The daemon runs as `debuggatha serve --root <path>` and speaks over **stdin/stdout**: UTF-8, **one JSON object per line** (NDJSON), `\n`-terminated, no newline inside a message.
- **stdout carries protocol messages only.** stderr is free-form diagnostics, never parsed.
- A line may be at most **16 MiB**. A larger message is a protocol error (`invalid_request`) and the connection is closed. Large results are not inlined: findings live in the ledger and are read in pages (section 5).
- The daemon is bound to **one repository root**, fixed at start. It is not changeable by a request.
- EOF on stdin means shutdown: the daemon cancels running work, stops child processes and exits. A `shutdown` request does the same after answering.

## 2. Messages

Every message has `"v": 1` and a `type`.

```json
{"v":1,"type":"request","id":"c-1","method":"review/run","params":{}}
{"v":1,"type":"response","id":"c-1","ok":true,"result":{}}
{"v":1,"type":"response","id":"c-1","ok":false,"error":{"code":"invalid_request","message":"…","errorId":"e-7f3a"}}
{"v":1,"type":"event","method":"progress","params":{}}
```

- **Either side may send requests.** Ids are strings; the client uses `c-<n>`, the daemon `s-<n>`, so the two spaces never collide. A response echoes the id of its request.
- **Events** have no id and no reply.
- Responses may arrive in any order and requests may run concurrently, except where section 6 says otherwise.
- **Tolerant reader:** a receiver ignores unknown fields in protocol messages, so a minor addition does not break an older peer. (Stored files are different: they are validated strictly; see `schemas/README.md`.)

## 3. Handshake and versioning

The first message must be the client's `hello` request; any other request before it fails with `not_initialized`.

```json
{"v":1,"type":"request","id":"c-1","method":"hello","params":{
  "client":{"name":"debuggatha-vscode","version":"0.3.0"},
  "protocol":{"min":1,"max":1},
  "capabilities":{"sampling":false,"progress":true}}}
```

```json
{"v":1,"type":"response","id":"c-1","ok":true,"result":{
  "server":{"name":"debuggatha","version":"0.3.0"},
  "protocol":1,
  "root":".",
  "capabilities":{
    "analyzers":["biome","gitleaks"],
    "semantic":[]}}}
```

- `protocol` is an integer major. The daemon picks the highest value both sides support; if none, it answers `protocol_mismatch` (with the versions it supports) and exits.
- Within a major version only additive changes are made (new optional fields, new methods, new error codes). Anything else is a new major.
- The client also checks the server **package** version against its own supported range and tells the person which side to update. Client and daemon versions are released in lockstep (ADR-0006), but the handshake is what enforces it.
- `capabilities.analyzers` and `capabilities.semantic` report what the **person who started the daemon** enabled (section 7). A client cannot enlarge them.

## 4. Methods (client to daemon)

The method set mirrors the MCP tools, because both are views of the same engine operations. Parameter and result shapes are the schemas named in the last column.

| Method | MCP tool today | Purpose | Shapes |
|---|---|---|---|
| `hello` | | Handshake | section 3 |
| `shutdown` | | Drain and exit | none |
| `review/run` | `review_changes`, `review_diff`, `review_files`, `review_workspace` | Run a review of a scope | params: `review-job`; result: `review-job-result` |
| `review/resume` | (new) | Continue a review that answered `input_required` | token + responses |
| `review/cancel` | | Cancel a running review (also available as the `cancel` event) | `{ "target": "<request id>" }` |
| `findings/list` | `list_findings` | Page through the ledger | filter + cursor; entries per `ledger` |
| `findings/get` | `get_finding` | One ledger entry with history | `ledger` entry |
| `findings/explain` | `explain_finding` | Rule, evidence and policy behind a finding | `finding` |
| `findings/update` | `update_finding` | Move a finding through its lifecycle (state machine enforced) | id + target status + comment |
| `findings/suppress` | `suppress_finding` | Record a suppression with a reason in Repository Memory | id + reason |
| `baseline/create` | `create_baseline` | Record today's findings as accepted | `baseline` |
| `repository/context` | `repository_context` | Capabilities, criteria, documentation context | output of Core Skills |
| `repository/summary` | `repository_summary` | One-line understanding of the project | text |
| `repository/policy` | CLI `policies` | The Review Policy a review would assemble (params: optional `packIds`) | `{ policy, requestedPackIds }` |
| `analyzers/list` | `list_analyzers` | What is installed and what is enabled | `analyzer-run` entries |
| `memory/list`, `memory/transition` | CLI `memory` | Repository Memory lifecycle | `memory-store` |

`review/run` returns `review-job-result` (summary, `changes`, analyzer and semantic records, an optional `findingsPage`, and `coverage`) or `ReviewInputRequired`. **`coverage` states what the review could not check** (languages without a parser, files parsed with errors, rules with no detector or needing type information, skipped files); `coverage.complete` is the single flag a client can test; a client must show coverage next to the findings (per-file detail on request with `coverageDetail: "files"`), never present an empty list as "clean". `review/run` params follow `review-job`; a `files` scope may carry `owned: true` to leave out what the project does not own (dependencies, build output), for a client that reviews on save and cannot know the scan scope (when nothing is left the request fails with `nothing_to_review`); (scope from `review-scope`, pack ids, analyzer ids, optional semantic request). Today's `ReviewJobResult` returns counts and relies on the ledger for findings; the daemon keeps that shape and adds `findingsPage` for clients that want one round trip (the MCP server).

## 5. Large results

- `review/run` returns the summary, the `changes` (introduced, fixed, reopened), analyzer and semantic run records, and the number of findings. It includes at most `limit` findings (default 100, highest severity first) with `truncated: true` when there are more.
- `findings/list` takes a `cursor` and returns `nextCursor`. Order is stable across pages for a given ledger revision.
- Finding ids and session ids in the current engine are random (UUID). Clients must not use them for equality across runs; the stable identity is the ledger fingerprint (file, rule id, content anchor).

## 6. Concurrency and cancellation

- Read methods (`findings/list`, `findings/get`, `repository/*`, `analyzers/list`) run concurrently.
- Methods that write the ledger, baseline or memory are **serialized per root**. A second review requested while one is running waits in order; it does not interleave writes. (Writes are atomic regardless.)
- `cancel` is an event `{ "target": "<request id>" }` (or the `review/cancel` request). The daemon stops the work, **kills any analyzer child processes**, and then answers the original request with `cancelled`. It never answers `cancelled` while work is still running.
- A cancelled review must not leave the ledger half-updated: the sync step either completes or does not start.

## 7. Trust boundaries across the protocol

These rules carry the CLAUDE.md boundaries over a process boundary; a client or a request can never relax them.

1. **Analyzers.** `safe` analyzers run when requested. `runs-project-code` and `network` analyzers run only if the **daemon was started with them enabled** (flag, environment variable or user-scope settings of the person), as reported in `capabilities.analyzers`. A request that names a disabled analyzer gets a skipped record (`status: "skipped"`, reason "not enabled"), never an error and never execution. Repository files cannot enable them.
2. **Semantic layer.** Off unless the daemon was started with it. Providers are `localhost` runtimes, or the client's own model via the server-initiated request below. A request flag `semantic: true` is honored only if `capabilities.semantic` is non-empty. Lines that may hold a secret and secret files are never sent. A model claim is kept only if it quotes the line it is about and the quote is on that line (the engine checks; the protocol only carries it).
3. **Client-supplied model.** Only if the person started the daemon with `serve --semantic host` and the client announced `capabilities.sampling`, a review with `semantic: true` sends the client a request `model/complete` (`params`: `purpose`, `system`, `user`, `maxTokens`; the client answers `result`: `text` and optionally `model`). `capabilities.semantic` then reads `["host"]`; a client cannot turn the pass on by announcing sampling alone. An answer takes at most two minutes; a cancelled review stops waiting. The client may refuse (`declined`); the review then continues without the semantic pass. The daemon sends nothing to any model itself in this mode.
   - **Adapters that cannot receive server-to-client requests.** MCP protocol 2026-07-28 forbids them and deprecates sampling. A client that announces `capabilities.inputRequired` (and not `sampling`) gets a second form. A `review/run` with `semantic: true` answers `{ "status": "input_required", "token": "<opaque>", "inputs": [ { "id": "i-1", "kind": "model/complete", "params": {} } ] }` when the review needs the model to read something, and the client continues with `review/resume { "token": "…", "responses": { "i-1": { "text": "…", "model": "…" } } }` (or `{ "declined": true }`). The answer to the resume is the end of the review or the next `input_required`, each with a new `inputs` id and the same token; one prompt is asked at a time. The review is suspended meanwhile and its ledger write has not started. A token lasts two minutes without an answer, after which the review is dropped and writes nothing. A resume checks that the files of the repository and the ledger are the ones the review began with; otherwise it answers `conflict` and the review is dropped. An unknown or spent token is `not_found`. While a review that persists waits, methods that write wait too, as they do for any review. A client that is stateless says what it can answer with each call: `review/run` takes `inputRequired: true` for that call, whatever the handshake said. The MCP adapter maps this to the input-required result and retry of MCP 2026-07-28 (`inputRequests` with `sampling/createMessage`, `requestState` for the token, `inputResponses` for the answers); the state is a random key held by the adapter and bound to the tool and its arguments.
4. **Paths.** Every path in a request is repository-relative and resolved against the root; anything that escapes it is `path_outside_root`. Git refs are validated and passed to git without a shell (`invalid_ref`).
5. **Errors.** Messages never contain absolute paths, stack traces, environment values or secrets. The daemon logs the detail to stderr and returns an `errorId` that correlates with that log line.
6. **Redaction.** Excerpts are redacted before they enter any response, the ledger or a model request.

## 8. Events (daemon to client)

| Event | Params | When |
|---|---|---|
| `progress` | `{ "request": "<id>", "stage": "orient\|detect\|analyzers\|semantic\|sync", "done": 3, "total": 120, "message": "…" }` | During `review/run`. Throttled to at most a few per second. Stages follow the order of Core Skills first, then review. |
| `log` | `{ "level": "info\|warn", "message": "…" }` | Optional human-readable notices. Never required for correctness. |

## 9. Error codes

| Code | Meaning |
|---|---|
| `protocol_mismatch`, `not_initialized` | Handshake problems |
| `invalid_request` | Malformed message or params that fail their schema |
| `unknown_method` | Method not in this protocol version |
| `path_outside_root`, `invalid_ref` | Rejected input |
| `not_found` | Unknown finding or memory id |
| `conflict` | Illegal lifecycle transition, or ledger changed incompatibly |
| `unavailable` | A required tool or model is not available |
| `declined` | The client refused a `model/complete` request |
| `cancelled` | The request was cancelled and its work has stopped |
| `internal` | Anything else; carries an `errorId` |

## 10. What this draft leaves open

- Exact params and results of `findings/*`, `memory/*` and `repository/*` beyond the shapes named above. They are specified in phase 5 against the CLI and MCP behavior, with conformance tests.
- The CLI's `--format json` report has no named type today: its envelope (`status`, `reviewResult`, `syncReport`, `suppressedFindings`, `suppressedInline`, `baselined`, `analyzers`, `semantic`, `countedFindings`, `persisted`) is assembled inline in `commands/review.ts`. The Rust CLI should give it a typed definition and a schema; this is a gap, not a decision.
- Whether the daemon also offers Streamable HTTP for hosts that cannot spawn a process. The MCP server keeps its own HTTP transport; the engine protocol stays stdio-only for now.

---

*Last updated: 2026-11-19*
