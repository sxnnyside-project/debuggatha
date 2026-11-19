# Roadmap: from a regex reviewer to a native, syntax-aware one

<!-- The plan, gates and record of the move to the native engine; for maintainers. -->

This plan implements [ADR-0006](adr/0006-native-rust-core.md). It has no dates: each phase ends at a **gate**, and the next phase does not start until the gate is met. Sizes are relative (S, M, L, XL), not time.

---

Ground rules for every phase:

- The deterministic core never depends on a model; trust boundaries in CLAUDE.md are unchanged.
- Every finding keeps a citable source. A detector that cannot tell says "not verified" instead of guessing.
- Every review states what it could not check: languages with no parser, and rules with no detector. "0 findings" must never read as "clean" when nothing was checkable.
- A published version is never broken: the TypeScript engine stays the shipped engine until the cutover gate (phase 8).
- No detector is "done" without the quality bar below.

## Quality bar for a detector

A detector, query or analysis stage is done when it has:

1. A **positive corpus** (code that must be reported) and a **negative corpus** (look-alikes, strings, comments, generated code, test code, other frameworks that must not be reported), kept as fixtures.
2. A **measured false-positive rate** on the real-repository corpus (phase 0), recorded in the module README with the corpus commit ids. Medium-and-above confidence needs a target set per rule before merging; low confidence is allowed only when the limit is stated in the rule's `limitations`.
3. An honest **confidence**: `high` only when the syntax tree plus resolved facts make the claim certain; unresolved facts (an imported helper that cannot be followed, a value of unknown origin) produce "not verified", not a finding.
4. Its limits written in the pack knowledge entry and the module README.

## Phase 3R: language-agnostic engine (2026-11-13, supersedes the order below)

The native engine was built JavaScript-first, which is a defect, not a scope: 32 of 333 pack rules have a detector,
24 of those only for JavaScript and TypeScript, and the analysis engine knows npm and Cargo only. Debuggatha reviews
any language; the Rust engine is rebuilt so that no detector or analysis is written against one language's syntax
tree. It works from phase 0 forward in Rust only (the TypeScript engine is not touched).

- A language-neutral semantic model (scopes, bindings, functions, calls, regions, literals, imports, declarations)
  is the only thing detectors and analyses read. Per-language adapters fill it; for tree-sitter languages an
  adapter is data (queries and node tables), and oxc is one adapter among the others.
- Rules name concepts (`process-abort`, `dynamic-eval`, `shell-sink`, `io-call`...) that data tables bind to each
  language and framework; "not applicable" and "not implemented" are different entries.
- Module boundaries and import resolution come from ecosystem descriptors (npm, Cargo, Go modules, Maven/Gradle,
  Composer, pub, NuGet, SwiftPM, mix, pyproject), not from npm and Cargo only.
- A stage is done when it holds for every tier-1 language. Differential parity with the TypeScript engine is a
  regression check for JavaScript, never the scope of a stage.

**Status: done.** One language-neutral semantic model (calls, functions, loops, tests, bindings, flow, imports, declarations) filled by 13 tree-sitter query adapters and an oxc visitor, which also reads the scripts of Vue, Svelte and Astro. Rules name concepts (`data/concepts.toml`) that tables bind per language: `no-eval`, `no-panic`, `no-shell-interpolation`, `n-plus-one-queries`, `a03-injection` and eight rules for languages that had no pack. Ecosystem descriptors (`data/ecosystems.toml`) give modules, imports and declared dependencies for npm, Cargo, Go, Python, Dart, Composer, Mix, Maven, Gradle (Kotlin DSL), .NET, Bundler and SwiftPM; dead code and the public surface are read from declarations. All 37 packs ship as TOML in `crates/debuggatha-engine/data/packs`. Rules about a feature of one language (`require-await`, `no-floating-promises`, `leak-mock-restore`) and the request-shaped taint stay on oxc; the differential results are in `docs/DETECTOR_QUALITY.md`. Open: the Gradle build in Groovy (no grammar in the engine), recall on real code (the corpus has little vulnerable code outside JavaScript), packs for GDScript, and detectors for the rules of the packs that have none (the coverage report lists them).

Phase 4 cuts 4 to 7 follow once cut 3 reads the model.

## Phase 0: freeze, measure, de-risk (M)

Goal: know exactly what "same behaviour" means, and retire the biggest unknowns before writing the port.

- **0.1 Golden corpus.** Run the current TypeScript engine over: this repository, a synthetic fixture repo per pack, and a **real-repository corpus**. The corpus is a **manifest** (`corpus/manifest.json`: repository URL, pinned commit, license, languages), not vendored code: third-party sources are fetched on demand into a scratch directory in CI and locally, treated as untrusted data (no install scripts, nothing executed), and only the manifest and the produced findings are committed. Selection follows the language tiers below. Store findings, ledgers and policy ids as JSON. Include non-ASCII sources and CRLF files.
- **0.2 Wire contracts.** Specify `Finding`, `ReviewJob`, `ReviewJobResult`, ledger/baseline/memory schemas and Protocol v1 (NDJSON; `hello` with version range; request ids; requests in both directions for sampling; cancellation; progress). Output: JSON Schemas in `docs/` and a protocol document.
  - Status: schemas in [schemas/](schemas/README.md) (generated from the 0.2.0 types, validated against real reports, ledger, baseline and memory files, with negative controls) and [PROTOCOL.md](PROTOCOL.md) (draft). Open: the CLI JSON report has no named type; ledger states other than the ones produced by a first review were not exercised by real data; `review-job` was not checked against live VS Code worker traffic.
- **0.3 Spikes with exit criteria** (each ends in a written yes/no):
  - JS/TS scopes and imports: `oxc_semantic` versus a purpose-built layer on tree-sitter. Criterion: resolves the taint and helper-resolution fixtures with no false resolution.
  - Rust MCP SDK: stdio, Streamable HTTP, sampling round-trip.
  - Tree-sitter grammar crates for tier 1 and tier 2 languages (below): load, query, cross-compile.
  - Cross-compilation for the five release targets, including C grammar builds.
- **0.3 outcome:** all four spikes answered yes, with the conditions listed under the results below. ADR-0006 is updated accordingly.
- **0.4** Done 2026-11-10: ADR-0006 accepted; CLAUDE.md has a "Direction" section and a note on sampling. Its package map and toolchain sections still describe the TypeScript code as it is, and are rewritten at cutover (phase 8), not before.

### Phase 0.1 baseline (measured 2026-11-10, TypeScript engine with the phase 1 detectors)

Reproduce with `node scripts/parity/fetch-corpus.mjs <dir>` then `node scripts/parity/run-corpus.mjs <dir> <out>`; reference files are in `corpus/golden/ts-engine/` (a projection without random ids or source excerpts) and `corpus/baseline-ts-engine.json`.

- 61 repositories (35 organization, 26 third-party): 61 completed, none crashed or timed out, 7,736 findings, 17.4 s in total; slowest repository 2.7 s. Two consecutive runs produced byte-identical files.
- Only 17 distinct rules produced findings. The largest: `no-var` (1,988), `no-explicit-any` (1,835), `no-unwrap-expect` (1,654), `strict-null-checks` (874), `prefer-template-literals` (687).
- **Coverage is silent.** Reviewing Flask prints "Review complete. 0 findings." with only the language-agnostic packs applied and no Python detector at all. Zero findings in Python, Swift, Java, Ruby and C++ repositories means "nothing checkable", not "clean". This violates the ground rule above and is the first thing the new engine must fix.
- **Precision samples (read by hand):**
  - `no-empty-tests`: 2 findings, both real (empty tests in zod).
  - `no-assertionless-tests`: 255 findings (zod 178, SvelteKit 38, Hono 16, Fastify 13). The samples are mostly not defects: tests whose assertion is that `parse()` does not throw, assertions inside a helper (`differential(...)`, `test_make_package(...)`), and `t.plan(n)`-style runners. This is the helper-resolution limit measured on real code; the rule needs resolved callees and a decision on implicit-throw assertions before it can leave low confidence.
  - `no-hardcoded-secrets`: false positives in three classes: localized UI strings with accents (`confirmPassword: 'Confirmar Contraseña'`), JWT fixtures in test files, and PEM header strings used to build keys (`append("-----BEGIN PRIVATE KEY-----\n")`). A real-looking Google API key in an OkHttp sample was reported correctly.
  - Environment drift (9 findings): two false-positive classes, variables set by tools (`VITEST`) and optional reads guarded later (`const custom = process.env.X; if (custom)`).
- Random ids (`id`, `sessionId`) appear in reports, so conformance compares the projection (rule, file, lines, columns, severity, confidence, category, pack), not raw output.

### Phase 0.3 spike results (2026-11-10)

Scratch projects, not in the repository; every number below was measured on this machine (Apple silicon, Rust 1.93 and 1.99.1) against the 61-repository corpus.

**Spike A: syntax trees (tree-sitter 0.25, Rust crates).** Verdict: **yes, for every tier 1 language and most of tier 2, with the limits below.**

| Grammar | Files (third-party / organization) | Parse without error nodes | Notes |
|---|---|---|---|
| JavaScript | 1,827 / 22 | 99.9% / 100% | |
| TypeScript, TSX | 1,617 / 679 and 68 / 1 | 98.0% / 99.9%, 98.5% / 100% | Grammar crate is from Nov 2024: variance modifiers (`out T = unknown`) fail, which hits zod v4 (33 files) |
| Python | 1,352 / 193 | 99.9% / 100% | |
| Go | 137 / 0 | 99.3% | |
| Rust | 507 / 516 | 97.0% / 100% | All 15 failures are one pattern: `str![[r#"..."#]]` snapshot macros |
| Dart | 964 / 29 | 98.0% / 100% | Failures are Mustache templates named `.dart`, not Dart |
| Elixir | 209 / 0 | 99.5% | |
| PHP | 3,188 / 160 | 99.0% / 100% | `Double::for(...)`: a reserved word as a method name |
| Kotlin | 672 / 50 | 98.7% / 100% | |
| C# | 549 / 9 | 99.5% / 100% | |
| Java | 125 / 100 | 99.2% / 100% | |
| Swift | 117 / 79 | **94.0%** / 98.7% | `#error(...)`, `.success(())` |
| C++ | 58 / 69 | **58.6%** / 94.2% | Macro-heavy code (fmt: `FMT_BEGIN_NAMESPACE`, `FMT_ENABLE_IF(...)`), C++26 reflection. Tree-sitter does not run the preprocessor |
| Ruby | 153 / 22 | 98.7% / 100% | |
| GDScript | 69 / 0 | 98.6% | |
| HTML | 534 / 7 | 98.5% / 100% | Failures are EJS/Jinja templates |
| Svelte | 1,021 / 83 | 99.5% / 98.8% | |

- All 18 grammars load and compile in 9 s. Throughput 2.4 to 29 MB/s per core depending on the grammar and corpus (Svelte on the third-party set is the slowest).
- **Vue and Astro have no usable grammar crate** (`tree-sitter-vue` last published 2022, no Astro crate). Extracting the `<script>` blocks (and the Astro frontmatter) with the HTML grammar and parsing them as TypeScript or JavaScript worked on all 1,074 script blocks found in 1,545 Vue, Svelte and Astro files: 100% clean. Templates are not parsed.
- **Queries:** environment-read queries written for Rust, Java, PHP, Kotlin, C#, Dart, Ruby, Elixir and Swift all returned the right count on a sample that mixes real reads with the same text inside comments and strings. On the corpus they match the text search within 2 results, except Elixir, where the query found 13 and the text search 33: the 20 extra are inside heredocs and sigils (examples and code templates), which is the point of using a tree.
- Behavior on a file with error nodes must be defined (phase 3): detectors run on the intact subtrees, the report says the file was parsed with errors, and nothing is concluded about the broken region.
- **C++ does not meet the bar as is** (58.6% on macro-heavy code). It stays "supported with a stated limit" and does not gate the migration.

**Spike B: JavaScript/TypeScript semantics (`oxc` 0.158).** Verdict: **yes, use oxc for the JS/TS family instead of tree-sitter.**

- Three fixtures, all passing: taint to `exec` (7 of 7, including a shadowed `const` and a block-scoped declaration that a name-based lookup resolves wrongly), helper resolution for tests (8 of 8, including a locally shadowed helper, an imported helper followed through `oxc_resolver`, and an uninstalled package reported as unknown instead of guessed), and environment reads (3 of 3, including an alias and a shadowed parameter).
- It parses and builds scopes at 26 to 31 MB/s over 4,186 JS/TS files and rejects only 3 (0.07%), against 2.0% for the tree-sitter TypeScript grammar on the third-party TypeScript files.
- Costs: the crates are pre-1.0 with breaking changes between versions I saw (string literals became a UTF-16-aware type, the parser's return struct changed, the semantic builder does not build AST nodes unless asked), and version 0.158 needs Rust 1.97. Pin exact versions, put them behind an internal adapter, and upgrade on purpose.
- Consequence for the design: JS/TS detectors are Rust code over oxc's tree and symbols, not `.scm` queries. Other languages use tree-sitter queries. Both produce the same finding and evidence types.

**Spike C: Rust MCP SDK (`rmcp` 3.9.0).** Verdict: **yes for stdio and Streamable HTTP; sampling works but is being removed.**

- A server with two tools served a client over a real child-process stdio transport and over Streamable HTTP on localhost; both negotiated protocol 2025-11-25 and completed tool calls, and a tool that asks the client's model through sampling got its answer back on both transports.
- **The MCP specification version 2026-07-28 forbids server-to-client requests, and sampling is deprecated (SEP-2577).** The SDK keeps sampling for older negotiated versions and adds a multi-round-trip mechanism instead (a tool returns an "input required" result and the client retries the call with its answers). Debuggatha's plan to reach "the client's own model through sampling" over MCP therefore needs a second path before hosts move to the new version. This is recorded in PROTOCOL.md; I did not build or test the new mechanism.
- Binary size of the MCP server with stdio and HTTP: 5.7 MB stripped.

**Spike D: cross-compilation.** Verdict: **yes.**

- The 18-grammar project, with its C sources, built from one Apple-silicon laptop for macOS arm64 and x64, Linux x64 and arm64 (GNU) and Windows x64 (GNU) using `cargo-zigbuild` and Zig, in a scratch environment (none installed globally). The outputs have the right formats.
- Not verified: the Linux and Windows binaries were not run (the Docker daemon was off and there is no Windows machine here); the Windows MSVC target was not built; the release path is still a CI matrix on native runners, which I could not exercise without pushing.
- Size: 27.6 MB for the 18 grammars (the parse tables are the cost and do not shrink with stripping or LTO); with oxc (2.4 MB) and the MCP server (5.7 MB) a combined binary is estimated at about 36 to 40 MB, against 85 MB for the current Bun executables. That sum is an estimate, not a measured build.

Gate: golden corpus checked in and reproducible; schemas reviewed; every spike answered; ADR accepted. Status: met, except that the schemas were reviewed and corrected only by the assistant (2026-11-10, see `schemas/README.md`, "Review pass"); a human read is still pending.

## Phase 1: Rust workspace and gates (S)

- Cargo workspace (`debuggatha-core`, `debuggatha-engine`, `debuggatha-cli`; internal crates `publish = false`), pinned toolchain, `rustfmt`, `clippy -D warnings`, `cargo-deny`, `cargo test`.
- `just check` runs both toolchains and stays the single CI gate; Husky pre-commit unchanged.
- Contract generation (`schemars`, `ts-rs`) wired into the build, with a CI check that generated files are current.
- Release pipeline dry run: build the four targets (darwin-arm64, linux-x64, linux-arm64, windows-x64; Intel macOS is not shipped), `cargo publish --workspace --dry-run` (nothing is published in this phase), package the npm launcher locally.

Gate: a no-op `debuggatha --version` binary builds, passes `just check`, and cross-compiles for every target in CI.

Status: done and run locally: the Cargo workspace under `crates/` (toolchain pinned, MSRV, edition 2024, workspace lints), `just rust-check` inside `just check` (fmt, clippy `-D warnings`, tests, `cargo-deny`), the contract pipeline proven on `Severity` and `Confidence`, and `cargo publish --workspace --dry-run` for the three crates. Nothing is published. Written and not yet run: the CI jobs `rust-targets` and `rust-publish-dry-run` and the Dependabot `cargo` entry. Not done: the CI freshness check of every schema, `ts-rs` types and the npm launcher.

## Phase 2: core domain port (L)

Port, module by module, with the conformance suite running against each:

- Findings ledger: state machine, matching, atomic writes, append-only history, summaries.
- Fingerprint: byte-identical to the current one (UTF-16 slice semantics, redaction before hashing). **Conformance includes real ledgers with non-ASCII excerpts.**
- Repository memory (lifecycle; only `active` filters findings; only a person confirms).
- Knowledge system: packs loaded from JSON, capability registry, policy resolution with deterministic ids (ADR-0005) and recorded conflicts (ADR-0003).
- Repository intelligence: stack detection with evidence, criteria resolution, documentation context, repository understanding.
- Review engine types and finding construction.

Gate: for every golden case, policy ids, ledger transitions and fingerprints are identical to the TypeScript output (differential test; any intended divergence is listed with its reason in a reviewed file).

Status: **gate met.** The ledger (lifecycle, matching, persistence), repository memory, the knowledge system (packs, policies, deterministic ids), repository intelligence and the review request, lifecycle and session types are ported, each against fixtures produced by running the TypeScript code (`scripts/parity/gen-*-fixtures.ts`). On the 61-repository corpus the repository context, policy and policy id (`corpus_gate`, opt-in: it needs the clones) and the ledger fingerprints, transitions and persisted text (`ledger_corpus_gate`, in `cargo test`) are identical to the TypeScript output, apart from the divergences in `docs/PARITY.md`. JSON Schemas for `finding`, `ledger`, `repository-context`, `review-request` and `review-session` are generated from the Rust types and a test fails when they are stale; the other seven are hand-reviewed snapshots until their engine types exist. Open: the pack format as files (done for the shipped packs in `data/packs`) and `just corpus-gate` in CI.



## Phase 3: syntax layer (XL)

- Tree-sitter integration, query runner, inline suppression directives, comment/string awareness (replaces `lex.ts`).
- Languages, by what the four organizations behind Debuggatha actually use (all active repositories, public and private, in `sxnnyside-project`, `sxnnyside-scholarships`, `core-red-project` and `devil-doll-entertainment`, by combined source size) and by which packs exist:
  - **Tier 1** (about 1 MB or more combined): TypeScript/JavaScript (+TSX/JSX) ~10.8 MB, Rust ~5.1 MB, Dart ~4.2 MB, Elixir ~1.3 MB, Python ~1.0 MB, PHP ~1.1 MB, and the component languages Vue ~1.1 MB and Astro ~1.9 MB (script and template embedding).
  - **Tier 2** (100 KB to 1 MB): Swift, Svelte, Kotlin, C++, Java, C#, Ruby, and GDScript (Godot).
  - **Tier 3** (a pack exists but the organizations do not use it, or use is negligible; validated on the public corpus only): Go, Visual Basic .NET, Assembly.
  - Packs that exist today: JavaScript, TypeScript, Node.js, Bun, Express, Fastify, Hono, Elysia, React, Vue, Svelte, Astro, Lit, Electron, Tauri, Rust, Go, Dart, Flutter, Kotlin, PHP, Laravel, ASP.NET, plus the concern packs.
  - **Languages in use with no Review Pack:** Python, Elixir, Swift, C++, Java, Ruby, GDScript. Syntax-level detectors from the language-agnostic packs (OWASP, testing, DX, performance) apply to them once their grammar is supported; language-specific packs for them are a follow-up to the migration, not part of it.
  A language is "supported" only when its load test and query corpus pass in CI. JavaScript, TypeScript, TSX and JSX use oxc; Vue, Svelte and Astro use the HTML grammar to extract script blocks and then oxc; C++ is supported with a stated limit (macros), and its parse-error rate is published.
- Port the existing detectors as queries or Rust code, keeping their tests as the spec.
- New detectors, built on the tree and measured on the real-repository corpus:
  - Tests: focused, empty and assertionless tests, with helper resolution across files; unhandled async in tests; spies left unrestored.
  - Environment drift: reads in every tier-1/2 language (`process.env`, `import.meta.env`, `os.environ`, `os.Getenv`, `env::var`, `getenv`, aliases and destructuring), compared with `.env.example`.
  - JS/TS taint to sinks (`exec`, `res.json`/`send`, SQL strings, `innerHTML`): known sources, propagation, sanitizer recognition; origin unknown is reported as "origin not verified".
  - Async misuse: `forEach(async)`, floating promises that are syntactically certain, `await` in loops (N+1), request-scoped state held in module scope, unvalidated `req.body`.
  - Error leakage, open CORS, rate-limit absence on model-calling endpoints, non-idempotent webhook handlers, destructive migrations (each with its own negative corpus).
- Anything that needs type information is documented as not covered.

Status: **built.** The syntax layer: oxc for the JavaScript and TypeScript family, tree-sitter for thirteen other languages, the HTML grammar plus oxc for the scripts of Vue, Svelte and Astro; a parse status per file (`Parsed`, `ParsedWithErrors` with the error spans, `Failed`), comment, string and regex facts, `mask_source`, a query runner and inline suppression (`debuggatha-ignore`). Parse rate on the corpus (12,859 files, opt-in `tests/corpus/syntax_trees.rs`, none failed): C# 99.5%, Elixir 99.5%, Go 99.3%, Java 99.6%, Python 99.7%, PHP 99.0%, Ruby 98.9%, Kotlin 98.8%, Rust 98.6%, GDScript 98.6%, HTML 98.5%, Dart 98.1%, Swift 95.9%, C++ 78.0% (no preprocessor; `Language::limited()`), JavaScript and TypeScript 98.6% to 100%. The detectors of the TypeScript engine are ported in five groups, compared detector by detector and then together against the golden findings; the new detectors (`require-await`, `no-loop-func`, `no-floating-promises`, `leak-mock-restore`, `n-plus-one-queries`, `a03-injection`, `validate-input`, `document-env-vars`) are measured on the corpus. Measurements and false-positive readings are in `docs/DETECTOR_QUALITY.md`; every divergence from the TypeScript engine is in `docs/PARITY.md`. A review states what it could not check (`coverage`); what needs type information is not covered.

Gate: quality bar met for every shipped detector; false-positive rates recorded; the 20-risk checklist (see below) mapped to detector, "delegated analyzer" or "not covered".

## Phase 4: analysis and engine port (XL)

- Skills: file, diff and architecture review on top of the syntax layer; Core Skills run first (CLAUDE.md).
- Analysis engine: module boundaries, dependency graph with **real import resolution** (relative, extension-less, dynamic and multi-line; workspace and path aliases where resolvable), layers and violations, cycles, public API surface, scope-aware dead code, ownership, change impact; unresolved edges are recorded, not guessed.
- Analyzers as separate processes with their trust classes: Biome, ESLint, Clippy, PHPStan, gitleaks, OSV-Scanner, plus `tsc` (`runs-project-code`); SARIF in and out.
- Semantic layer: unchanged trust model (off by default, `localhost` or client sampling, claim kept only if it quotes the line); context enrichment (enclosing function, imports, types of the symbols involved), and workspace scope support.
- Baseline, scan scope, secret redaction, git refs validated and passed without a shell, sanitized errors.

Status: cuts 1 (module boundaries, import resolution, dependency graph) and 2 (layers, violations, cycles) are built and described in `docs/ANALYSIS_QUALITY.md`. Cut 3 (ownership, public API, dead code, change impact) is built for every language that has a module reader: `analysis::analyze` picks the reader of each module, and what no stage reads (C++, GDScript, Rust dead code) is returned as not covered (`docs/ANALYSIS_QUALITY.md`). Cut 4 (unified diff parser, change impact down to the touched export statements) is built; cut 5 (diff review: git changes with validated refs and no shell, the files a change touches read whole with only added lines as findings, what could not be read returned) is built; cut 6 (baseline: create, apply, save, load and clear, read-compatible with the TypeScript file) is built; cut 7 (external analyzers as separate processes: trust gate, time and output limits, SARIF, merge with the built-in findings; all nine adapters of the TypeScript engine, including `equivalentTo`) is built. The semantic layer (verification and detection with a local Ollama or LM Studio or any provider that implements the trait; secrets withheld; a claim kept only if it quotes its line) is built. Errors for clients are classified by type (`client_error`): a filesystem cause gets a fixed message, an unvetted type says nothing. Not yet in Rust: the host's own model through sampling (the daemon, phase 5).

Gate: golden corpus passes for all review scopes; secret-redaction and path-sanitization tests pass; fuzz targets for the diff parser and the query runner run in CI.

## Phase 5: CLI and daemon (M)

- Command parity: `review`, `baseline`, `findings`, `memory`, `repository`, `packs`, `policies`, `analyzers`, `init`, `trends`, `doctor`, `completion`; exit codes unchanged (`0`, `2`, `1`).
- `serve` daemon on Protocol v1; `doctor` checks versions and tools; `mcp` subcommand if the spike passed.
- Benchmarks (`docs/benchmarks.md`) re-run; startup, whole-repository review and memory compared with the TypeScript engine.

Gate: end-to-end tests drive the binary, not a library.

Status: the CLI is complete. The review pipeline is in the engine (`pipeline::execute_review`: context, policy, detectors for workspace, files, diff and a diff handed over, analyzers, a local model, memory, baseline, ledger sync and what changed) and every command of the TypeScript CLI exists: `review` (scope by `--all`, `--files`, `--diff`, `--base`, `--changed-since` or the working tree; `--fail-on-new`; `--fail-on`; `--format text|json|sarif|github|markdown|gitlab`; `--output`; `--quiet`; `--no-persist`; `--include-baselined`; analyzer and semantic flags; the repository's `.debuggatha/config.json` for `ignore`, `overrides`, packs and analyzer limits), `baseline`, `findings`, `memory`, `repository`, `packs`, `policies`, `analyzers`, `init`, `trends`, `doctor` and `completion`, all with tests that run the binary. Exit codes `0`, `2`, `1`. `debuggatha serve` runs the engine over Protocol v1 (see `docs/PROTOCOL.md` for what is built); end-to-end tests drive the process. Cancellation and progress events are built. Benchmarks are recorded in `docs/benchmarks.md`: startup and one file favor Rust, the whole repository is still about 1.8 times slower than TypeScript. The client's own model is built over `model/complete` and over `input_required` with `review/resume`. `debuggatha mcp` serves the thirteen tools, three prompts and four resources over stdio and, with `--http`, over stateless Streamable HTTP on a loopback address (hand-written, tested against the TypeScript SDK's clients); over stdio the client's model is reached through sampling and progress goes to the call's progress token. The roots a client shares and a call's `rootDir` are honored within what the person allowed (`--allow-root`). The stateless era of MCP 2026-07-28 is served beside the earlier one: `server/discover`, `-32022` for an unsupported version, `resultType` on every result, and the input-required result and retry for a review that reads the code with the client's model (`-32021` when the client cannot sample). Not checked against a real 2026-07-28 client (no SDK release speaking it was available); the HTTP headers it requires (`Mcp-Method`, `Mcp-Name`, `MCP-Protocol-Version`) are not validated. Phase 5 is closed.

## Phase 6: adapters (M)

- MCP: tools, prompts and resources unchanged for hosts; engine out of the package.
- VS Code: runner replaced by a daemon client; `cliPath` is machine scope; handshake validates the version range; clear messages when the CLI is missing or incompatible; untrusted workspaces run built-in detectors only; integration tests on the minimum and stable VS Code versions still pass. The extension bundle drops the engine.
- Contracts package generated from Rust; adapters import it, never the engine.

Status: the VS Code extension is a client of the binary (2026-11-19). `apps/vscode/src/daemon` starts `debuggatha serve` (path from the machine-scope `debuggatha.cliPath`, else the PATH), does the handshake, checks the CLI version (`compat.ts`), speaks NDJSON, maps cancellation to the `cancel` event and progress to the notification, and restarts the daemon when a setting that decides how it starts changes (analyzers, model, trust, binary). The ledger, lifecycle changes, inline-accept comment, repository context and policy all come from daemon methods (`repository/policy` and `owned` on a files scope were added for it). The bundle is 31 KB with no engine and no worker; the types of what the daemon sends are generated from `docs/schemas` (`just vscode-contracts`). The unit tests drive the real binary, and the 19 integration tests pass in a real VS Code 1.90.2 and in the stable one, on macOS arm64 only. The extension does not bundle the CLI (that is phase 7); a missing or mismatched CLI is reported once, with the setting that fixes it. The MCP package of npm is now a launcher (`npm/mcp`, 0.3.0, no engine, no dependencies): `DEBUGGATHA_CLI`, then `@sxnnyside/debuggatha-<os>-<cpu>`, then `PATH`; the TypeScript server stays in `packages/mcp` as a private parity reference. The thirteen tools now declare `outputSchema` (top-level shapes), honest `idempotentHint`/`destructiveHint` (`suppress_finding` and `create_baseline` are destructive), preconditions and error recovery in their descriptions, and reviews take `limit`; this answers the third-party reports on 0.2.0 (missing output schemas and annotations, brief descriptions, no pagination). The per-platform packages do not exist yet, so until phase 7 the launcher needs the binary from `DEBUGGATHA_CLI` or `PATH`. Not done: no Linux or Windows run of the extension; no `log` event or `coverageDetail` use in the editor; the editor shows `coverage.complete` only as a sentence in the completion message.

Gate: adapter test suites pass against the real binary on all CI platforms.

## Phase 7: distribution (M)

- npm: `@sxnnyside/debuggatha-cli` and `@sxnnyside/debuggatha-mcp` become launchers; per-platform packages as `optionalDependencies`; no install scripts; provenance; lockstep versions.
- GitHub Releases and Homebrew with checksums; crates.io publish order and metadata per the Rust packaging checklist; the README only promises channels the release workflow performs.
- The release workflow validates tag/version agreement across npm, Cargo and the extension.

Status (2026-11-19): built, not yet run by a tag. `v*.*.*` tags run `release.yml`: version check on every manifest (`scripts/verify-version.sh`: the three crates, both launchers and their platform dependencies, `server.json`, the extension), the CI workflow as the gate, native builds for macOS arm64, Linux x64 and arm64 and Windows x64, then one publish job: `SHA256SUMS`, release notes from the Conventional Commits (`git-cliff`), npm (platform packages first, with provenance and a skip for a version already published), crates.io in dependency order, the Marketplace and Open VSX, and one GitHub release. `just smoke` installs the packed launchers and this platform's package into an empty project and runs `debuggatha --version` and an MCP handshake under Node and Bun (passes here, macOS arm64). `cargo publish --workspace --dry-run` passes. Homebrew builds from the tag in the tap, which bumps itself; nothing here notifies it. Not done: no tag has run the workflow, so npm provenance, the trusted-publisher setup on npmjs.com (the first release of each new package needs a token), the crates.io token and the extension tokens are untested; macOS Intel is not a target; a clean-machine `debuggatha doctor` per channel has not been run.

Gate: a clean machine installs through each channel and `debuggatha doctor` passes.

## Phase 8: cutover and hardening (M)

- Switch the published packages to the Rust engine in a new version of the same npm names; keep the previous TypeScript engine tagged for reference; fixes only.
- Dogfood: Debuggatha reviews itself and the real-repository corpus in CI; results published in the repository.
- Module READMEs, glossary, PACK_SPEC (data format, queries) and INTEGRATIONS updated; CLAUDE.md reflects the final layout.

Status (2026-11-19): the cutover itself is done in the tree. The TypeScript engine, its Bun and Turborepo tooling, its fixture generators and the license script are removed; `ts-engine-final` is the tag of the last commit that had them. The goldens and fixtures stay as the frozen contract (`docs/PARITY.md`). Gaps found by comparing the command lines, the variables and the tools with the TypeScript ones were closed (`-C/--cwd`, `--json`, the `DEBUGGATHA_*` variables, `semantic-eval`, the analyzer license test); the ones left are listed in `docs/PARITY.md`. Not done: dogfooding in CI, and the false-positive rates are not published.

Gate: conformance green, benchmarks recorded, false-positive rates published, `just check` green on all platforms.

## Mapping the 20 common risks

| # | Risk | Planned coverage |
|---|---|---|
| 1 | Hardcoded secrets | Exists (known token formats, entropy); gitleaks as analyzer |
| 2 | Unvalidated `req.body` | Phase 3 (framework-aware: validator recognized before use) |
| 3 | Floating promises, `forEach(async)` | Phase 3 (syntactically certain cases); type-dependent ones not covered |
| 4 | Command and SQL injection | Phase 3 taint |
| 5 | Hallucinated or abandoned dependencies | Opt-in `network` analyzer (registry lookup), enabled only by the person |
| 6 | Request state held in module scope | Phase 3 |
| 7 | Open CORS, insecure cookies | Phase 3 |
| 8 | Unsanitized HTML sinks | Exists for React/Vue; Phase 3 generalizes |
| 9 | N+1 queries | Phase 3 (`await` in loops over data-access calls) |
| 10 | `as any` / `@ts-ignore` | Exists (`any`); `@ts-ignore` added in phase 3 |
| 11 | Testing theatre | Phase 3 tests detectors |
| 12 | Non-idempotent webhooks | Phase 3 (handler heuristics, stated limits) |
| 13 | Error and stack leakage | Phase 3 taint |
| 14 | ReDoS, event-loop blocking | Phase 3 (known catastrophic patterns; sync I/O exists) |
| 15 | Phantom environment variables | Phase 3 (all tier-1/2 languages) |
| 16 | Large rewrites that drop edge cases | Out of scope for detection; diff review reports removed branches and tests |
| 17 | No rate limiting on model-calling endpoints | Phase 3 |
| 18 | Check-then-act races | Phase 3 (limited, syntactic; stated limit) |
| 19 | Focused tests committed | Phase 3 (already in the TypeScript engine) |
| 20 | Destructive migrations | Phase 3 (SQL and ORM migration files) |

## Risks

| Risk | Mitigation |
|---|---|
| Port stalls feature work | Phase gates; TypeScript engine stays shippable; conformance suite is the contract |
| Ledger fingerprints change | Byte-level conformance on real ledgers (phase 2 gate) |
| Grammar crate or tree-sitter ABI drift | Pinned in `Cargo.lock`; load test per grammar; upgrade is a deliberate change |
| Taint and scope work is larger than expected | Phase 0 spike decides `oxc_semantic` versus own layer before phase 3 starts |
| Release engineering for four targets | Phase 1 dry run in CI from the start |
| MCP SDK for Rust is not ready | Fallback: thin TypeScript client over the daemon (ADR-0006, decision 6) |
| False positives erode trust | Real-repository corpus; per-rule targets; "not verified" instead of low-confidence noise |

## Decisions

1. **Names:** `debuggatha-core`, `debuggatha-engine`, `debuggatha-cli` (all free on crates.io when checked).
2. **Packs:** JSON canonical, TOML as authoring form, same schema and same policy id (ADR-0006, decision 3).
3. **Languages:** the tiers above, set from the real use across the four organizations. Ruby, C++ and Elixir are included in the plan (Elixir is tier 1).
4. **Corpus:** a manifest of repositories at pinned commits, fetched on demand. Public manifest: public repositories only, including the organizations' own CLIs and libraries (for example the `core-red-project` CLIs) and public projects per tier. Private repositories (including all current Elixir code) may be used only through a **local, git-ignored manifest** run on the owner's machine, never in public CI, and with findings kept local (counts and rule ids, no excerpts). A public Elixir project is needed for the shared corpus. Nothing is cloned until a manifest is approved.
5. **TypeScript packages:** the `@debuggatha/*` packages were kept in step with the migration as the parity reference (ADR-0006, decision 8). At cutover the TypeScript engine is tagged and archived; the contracts package stays, generated from Rust.

---

*Last updated: 2026-11-19*
