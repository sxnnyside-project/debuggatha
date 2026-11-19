# Parity with the TypeScript engine

<!-- Every intended difference from the frozen TypeScript engine; for contributors changing engine behavior. -->

The TypeScript engine is gone from the tree (ADR-0006, decision 8): it is in the git history, tagged
`ts-engine-final`. What it left is frozen as the contract: the conformance fixtures in
`crates/*/tests/fixtures` and the goldens in `corpus/golden`, written by that engine's own code. A
module must give the same results; the tests in `crates/*/tests` compare with those files. Every
intended difference is listed here with its reason. A difference not listed here is a bug.

The fixtures cannot be regenerated any more. To change one on purpose, write the divergence here and
edit the expected output by hand, naming the case.

---

## What the command line and the MCP server took, and what they take now

| TypeScript | Rust |
|---|---|
| `-C, --cwd <dir>`, `--json` | The same spellings (`-C`, `--cwd` for `--root`; `--json` for `--format json`) |
| `DEBUGGATHA_REPOSITORY_ROOT`, `DEBUGGATHA_ANALYZERS`, `DEBUGGATHA_SEMANTIC` (`off`, `sampling`, `ollama`, `lmstudio`), `DEBUGGATHA_SEMANTIC_MODEL`, `DEBUGGATHA_SEMANTIC_URL` | The same variables, read by `serve` and `mcp` (and `--root`). `sampling` is a spelling of `host` |
| `DEBUGGATHA_ANALYZERS=off` | Not accepted: the analyzers that only read source always run for a client that does not turn them off (`analyzers: false` on a call, `--no-analyzers`). The tools that run project code or use the network stay off unless a person names them |
| `DEBUGGATHA_LOG_LEVEL`, `DEBUGGATHA_CACHE_ENABLED`, `--verbose` | Gone: the binary writes nothing to stderr beyond errors, and there is no cache to switch |
| `just semantic-eval` (the labeled set and its report) | `just semantic-eval`: the same set and report, as `cargo run --example semantic_eval` |
| `scripts/license-gate.mjs` (analyzer licenses, none bundled) | A test in `crates/debuggatha-engine/tests/analyzers.rs`; `cargo deny` checks the crates |

## Intended divergences

| Area | TypeScript | Rust | Reason |
|---|---|---|---|
| Reading `ledger.json` and `memory.json` | Accepts any JSON with a numeric `schemaVersion`; unknown fields pass through | Refuses a file with unknown fields or wrong shape (`Malformed`) | The TypeScript reader hands back a value typed as `Ledger` without checking it, and a later save would drop what it did not know. Failing loudly beats losing data. |
| Writing `ledger.json` | `writeFileSync` then rename | Also `fsync`s the file before the rename | A crash between write and rename could leave an empty file under the final name on some file systems. The bytes written are identical. |
| Writing `memory.json` | `writeFileSync` straight onto the file (not atomic) | Same atomic write as the ledger: temporary file, `fsync`, rename | The CLI, the MCP server and the editor share this file too; a cancelled write must not leave half a file. The bytes are identical. |
| Summaries | Plain objects (insertion order) | Sorted maps | The JSON is the same object; only key order differs, and summaries are not persisted. |
| Re-registering a pack (same id and version) | `listPacks` keeps the replaced pack and lists both | `list_packs` lists only the replacement | The TypeScript behavior is a leak: `getPack` already ignores the replaced pack, so the list and the lookup disagreed. |
| A dependency that declares both `packId` and `skillId` | `validateReviewPack` reports it | Cannot be written in a typed pack; refused when the pack JSON is read | Packs become JSON validated by a schema (ADR-0006, decision 3), so the malformed shape never reaches the validator. The empty and missing cases are still reported with the same messages. |
| `no-eval` and `no-shell-interpolation` in test files of PHP and the other non-JavaScript languages | Reported (PHP: 17 hits in the Laravel tests) | Skipped, as the JavaScript rules skip them | A test carries deliberately bad code by design; the concept table says so once (`skip_tests`) for every language. |
| A file named `FooTest.java`, `FooTests.cs`, `FooIT.kt`... | The suffix is matched without regard to case, so `Commit.java`, `Split.kt` and `Submit.java` (ending in `it`) are tests | Only a capital `Test`, `Tests` or `IT` after a letter or digit; also `test_*.py`, `conftest.py`, `*_spec.rb`, `*_test.rb`, `*_test.exs`, `*_test.cc` and `*_test.cpp`, and `Tests.swift` | A source file was skipped as a test because of its name; and the test files of Python, Ruby, Elixir and C++ were not recognized. |
| Frameworks of the non-JavaScript ecosystems | Only Laravel, Flutter and Tauri were read from manifests of those ecosystems | Django and Flask (`requirements.txt`, `pyproject.toml`, `Pipfile`), Rails (`Gemfile`), Spring (`pom.xml`, `build.gradle`, `build.gradle.kts`), Phoenix (`mix.exs`) and ASP.NET Core (`.csproj`) are detected from the text of the manifest | A pack that requires a framework no scan could name never applied (`aspnetcore` never did). The corpus gate leaves those frameworks out of what it compares. |
| Which paths are tests | The directories `test`, `tests`, `spec`, `androidTest`... | Also the Kotlin Multiplatform test source sets (`jvmTest`, `commonTest`, `iosTest`, `nativeTest`, `jsTest`, `desktopTest`, `wasmJsTest`) | A test source set is test code, as `androidTest` is. Found on a hit in `okhttp`. |
| Languages read from the manifests at the root | `Package.swift`, `mix.exs`, `CMakeLists.txt`, `*.csproj` and `*.sln` are not read | Swift, Elixir, C++ and C# are detected, with capabilities `swift`, `elixir`, `cpp` and `csharp` | A repository in those languages had no language capability, so no pack could apply to it. The corpus gate leaves those four out of what it compares and recomputes `understanding` without them (`without_newer_languages`); 61 of 61 repositories match. The "no recognized manifest" reason now lists the new manifests, and the gate ignores that text. |
| Reading the repository's directories | `src/` is listed with an unsorted `readdirSync`: the order of the evidence of `layered` and `feature-based` depends on the file system | Always sorted | A latent defect: the same repository could give two different contexts on two machines. The comparison sorts that evidence on both sides. |
| `declaredVersions` and `runtimeConstraints` | Copy whatever the manifest holds, even an object, against their own `Record<string, string>` type | A number or boolean is read as its text; an object or array is left out | The contract says strings. |
| A manifest that is not valid JSON | `buildRepositoryContext` throws a `SyntaxError` with V8's message | Returns `ScanError::InvalidJson` with the parser's message | Both fail; the text differs. A `package.json` that is `null` fails in both. |
| Masking code from comments and strings (JS/TS) | `lex.ts` is a line lexer: it hides a whole template literal (including `${...}`), reads an apostrophe in JSX text as the start of a string, and cannot tell a regex from a division except by heuristics | The parse decides: only the text parts of a template are hidden, JSX text is code, regex literals come from the tree | A match inside `${process.env.X}` was invisible before; an apostrophe in JSX text hid the rest of the line. Layout is otherwise identical (same length, UTF-16 columns). Three cases pinned in `tests/syntax/conformance.rs`. |
| `debuggatha-ignore` in a file with CRLF line endings | The lexer keeps the `\r` at the end of the comment line and the directive pattern's `.` refuses to match it, so the directive is silently ignored and the finding it accepted is reported again | The trailing `\r` is dropped before the directive is read | A suppression that stops working when a file has Windows line endings is a defect. It failed safe (findings came back, none were hidden), so the fix cannot hide anything. Pinned by `crlf_files_keep_their_directives_unlike_typescript`. |
| What a detector reads in a diff review | The added lines alone, as a fragment with a map of line numbers | The whole file, parsed once; only hits on added lines are kept | A fragment is rarely valid code, and a parser would report errors the file does not have. Hits on the same lines come out the same. |
| A file the parser cannot read at all | The lexer never fails: every detector runs on whatever it masked | Syntax detectors do not run; `Run.not_analyzed` lists the file; text detectors (secrets) still run | Nothing is concluded about a file with no tree. A `debuggatha-ignore` comment in such a file cannot be read and the finding is reported: failing safe. |
| An excerpt cut inside an astral character | JavaScript `slice` leaves a lone surrogate | The half is written as U+FFFD | Rust strings cannot hold a lone surrogate; Node hashes one as U+FFFD, so the ledger fingerprint is the same. |
| `no-var` | A line holding `var` and a name; also reports `declare var` and `var` inside `declare global`; misses `var { a } = o` and `var` at the end of a line | The declaration node: reports destructured and line-split `var`, never an ambient one | Corpus: +5 destructured (`var { status } = resp`), -7 ambient (`declare global { var $pinia }`). An ambient `var` is the only way to declare a global; it is not code. |
| `no-explicit-any` | `: any`, `as any`, `<any>` only | Every `any` type keyword; a line with an old-form match keeps its columns | Corpus: +251 (238 in forms the regex did not know: `Record<string, any>`, `() => any`, `FastifyRequest<any, any>`, `T = any`; 13 inside `${}`), -1 (`export { anyType as any }` is an alias, not a type). 1,834 hits are identical. |
| `strict-null-checks` | A name, `)` or `]` right before `!` | The non-null assertion node | Corpus: -102 (53 are `!` in JSX text such as `Hello!`, 49 are definite assignments such as `x!: T` and `let v!: T`), +9 (inside `${}`). 772 hits are identical. |
| `eqeqeq` | `==`/`!=` where `null` is not next to it | The binary expression; `null` in either operand (also parenthesized) is accepted | Corpus: +1, a line the lexer had lost (it was inside a "string" that never ended). 13 identical. |
| `no-eval` (JS/TS) | `eval(`, and `setTimeout`/`setInterval` with a string | The call to the global; a local `setTimeout` or `Function` is not it | No corpus difference in JS/TS. PHP is in the tree-sitter rules (below). |
| `prefer-object-spread`, `prefer-template-literals` | Per line: `Object.assign({}` and `"s" + name` / `name + "s"` | The expression, wherever it spans lines and inside `${}` | Corpus: +2 and +20 (10 inside `${}`, 8 split over two lines, 2 on lines the lexer had lost). 29 and 687 identical. |
| `no-extend-native` | `X.prototype.y =` and also `X.prototype.y ==` | Assignments only | 6 of 6 identical on the corpus; the comparison case is a fixture. |

| `no-async-foreach` | `.forEach(` followed by the word `async`, also a variable of that name | A call to `forEach` whose first argument is an async function | Fixture only: `items.forEach(async)` is a name, not a function. |
| `no-shell-interpolation` | `exec`/`execSync` whose first argument starts with a quote and holds a `+` anywhere | A template with `${}`, or a `+` chain that holds text and a value | `exec(cmd + " -v")` is reported now (the shell reads `cmd` either way); `exec("a" + "b")` is not (two constants). Fixtures only. |
| `no-error-leak` | `res`/`response`, optionally `.status(` with no `)` inside, then `.json(`/`.send(` with `err.stack` or `err.message` in the arguments | The same call shape on the tree | `res.status(code()).json(...)` and `err?.message` are reported now. Fixtures only. |
| `no-focused-tests` | `it.only(`, `fit(`... directly followed by `(` | A call whose callee chain starts at a test word and holds `only` | `describe.only.each(rows)(...)` and `test.describe.only(...)` are reported now. Fixtures only. |
| `no-open-cors` | Both `cors()` and `origin: '*'` on one line give two findings | One finding per line and rule, like every other rule | The first keeps its columns, so its ledger identity is unchanged. |
| `no-empty-tests`, `no-assertionless-tests`, `async-route-errors` | Bracket counting over masked text | The callback node: its block body, the names in it, a `try` or `.catch(` call | No difference on the corpus or in 100+ fixture cases; the cases cover nesting, comments, strings, `done` and helper names. |
The JavaScript syntax rules were compared on the whole corpus (`tests/corpus/parity_js_syntax.rs`, opt-in): **5,324 hits are identical in rule, file, line, columns, severity, confidence, category and excerpt**, and every difference is one of the classes above. The exact set is committed in `corpus/golden/divergences-js-syntax.json`; a new difference fails the test until it is looked at. `no-implied-eval` and `no-dangerously-set-inner-html` have no hits in the corpus and rest on their fixtures (`tests/parity/js_syntax.rs`, 107 cases).

| `no-unwrap-expect` (Rust) | `.unwrap()` / `.expect(` anywhere in a `.rs` file outside `tests/` paths, strings and comments | The method call node (and the tokens of a macro body, which the parser does not structure); code under `#[test]`, `#[tokio::test]` or `#[cfg(test)]` is test code | The rule never meant to flag tests, but only judged by path. Corpus: 475 hits identical; 1,637 of the TypeScript's 2,122 are in test items (the count with the test skip switched off matches the TypeScript on every other hit); 10 TypeScript hits are text inside a string literal that its lexer read as code, and 62 hits the TypeScript lexer lost (it lost state, as in the JavaScript syntax rules). |
| `no-panic` (Rust, Go) | `panic!(` / `panic(` outside test paths | The macro or call node; a Go declaration `func panic(` is not a call; Rust under `#[test]`/`#[cfg(test)]` is test code | Corpus: 83 identical, 42 in Rust test items. With the test skip off, 125 identical and none different. |
| `no-unsafe-blocks` (Rust) | `unsafe {` and `unsafe fn` on one line | The `unsafe_block` and `unsafe fn` nodes (a brace on the next line counts); tests are still reported, as before | Corpus: 66 identical. `unsafe impl`/`unsafe trait` are still not reported (parity, not a decision). |
| `avoid-print` (Dart) | `print(` after anything that is not a name character or `.` | A call to the identifier `print`; a declaration `void print(...)` is not a call | Corpus: 84 identical. The edit (`debugPrint`) is unchanged. |
| `no-eval` (PHP) | `eval(` after anything but `.`, name characters or `$`, so also `$redis->eval(` and `Foo::eval(` | The `eval` function call; a method or static method of that name is not the construct | Corpus: 18 identical; the 24 hits only the TypeScript reports are Redis `->eval(` method calls (Laravel's cache/queue/broadcasting). PHP names are case-insensitive but the old pattern was not, and neither is the new one: `EVAL(` is reported by neither (a gap, not a decision). |
| `use-v-html-carefully` (Vue) | `v-html`, spaces, `=` anywhere in a `.vue` file; the old lexer masked nothing in a component, so comments and script strings counted | An attribute named exactly `v-html` with a value, read from the HTML grammar | `:v-html`, `data-v-html`, a `v-html` in an HTML comment, in a script string or comment, or in a style block are not the directive; a `=` on the next line is. Corpus: 3 hits, identical; the corpus has no case of the differences, so they rest on the 7 differing fixtures of `tests/parity/vue_template.rs` (24 cases, 17 agree). The component is parsed again as HTML, because the SFC parse keeps only the script blocks. |
| `no-hardcoded-secrets` | Known formats on the raw line (kept); a literal assigned to a secret-looking name, with the literals from the TypeScript lexer, which knew no `.mts`, no components, and for a multi-line literal redacted nothing | The same two passes and the same judgments of the value (length, entropy, placeholders, words, URLs, paths), with the literals from the parse | **A value that runs over several lines is redacted at its first line.** The TypeScript excerpt kept the start of the secret (`` const password = `abcdefgh ``), so part of a credential reached the ledger; this excerpt, and so that finding's fingerprint, differs on purpose. `.mts` files and the `<script>` of Vue/Svelte/Astro are now judged. A literal with a prefix (`f"..."`, `r"..."`) is still not judged: the first version of the port judged it and reported two character tables on the corpus (`_tokenChars = r"..."`). A file the parser cannot read at all gets the known-format pass only. |
The JavaScript call and test rules were compared detector by detector (`tests/corpus/parity_js_calls.rs`, opt-in, with a dump of the TypeScript hits from `scripts/parity/dump-detector-hits.ts`, because the golden policies hold only some of them): **264 hits, all identical in rule, file, line, columns, confidence and excerpt, none only on one side** (255 `no-assertionless-tests`, 4 `async-route-errors`, 2 `no-empty-tests`, one each `no-error-leak`, `no-open-cors`, `no-shell-interpolation`). The corpus has no hit for `no-async-foreach` or `no-focused-tests`, and the other rules have a handful: the evidence for this group is the 149 fixture cases (`tests/parity/js_calls.rs`; 140 agree exactly, 9 differ and state why), not the corpus. The set is committed in `corpus/golden/divergences-js-calls.json`.


The rules of the tree-sitter languages were compared detector by detector (`tests/corpus/parity_tree_sitter_rules.rs`, opt-in, same dump script as the JavaScript call and test rules): every difference above is classified and the exact set is in `corpus/golden/divergences-tree-sitter-rules.json`. Fixtures: 68 cases in `tests/parity/tree_sitter_rules.rs`, 57 agree exactly and 11 differ and state why (test items, spaced dot, brace on the next line, declarations, PHP methods).


The secrets rule was compared on the whole corpus (`tests/corpus/parity_secrets.rs`, opt-in): **116 hits, identical in rule, file, line, columns, confidence, severity and excerpt, none on one side only**. The fixtures (`tests/parity/secrets.rs`: 68 cases, 64 agree, 4 differ) carry the rest: tokens in comments, examples, test files and fixtures, every judgment of a value, UTF-16 columns and CRLF. Known gap: a file with no parser (a `.scala`, a `.yaml` named on the command line) gets only the known-format pass, and the review does not yet say so.

| Review session transitions | `transitionSession(session, to, patch)` spreads `Partial<ExecutionMetadata>`; a key present with `undefined` would clear a field | `ExecutionPatch` fields are `Option`: `None` keeps the value, nothing clears one. `transition_session` takes the session by value | The engine (`pipeline.ts`) only ever sets fields; a clearing patch is not used and would hide a lost `error`. |
| Files a workspace scan reads | `ts tsx js jsx mjs cjs dart rs go py kt kts php cs java vue svelte` (`read-files.ts`) | The same list for the gates (`PARITY_EXTENSIONS`), and for a review of a workspace that list plus `rb ex exs swift cpp cc cxx hpp hh hxx gd astro` (`SCAN_EXTENSIONS`) | The engine parses those languages and `document-env-vars` reads five of them. Measured on the corpus (1,086 more files): one more finding, a `no-hardcoded-secrets` in a Ruby file; every other rule has no detector for them. `html` (518 files, no rule reads it) and `.h` (a header does not say its language) stay out. A file named on purpose was always read, whatever its extension. |
| Module boundaries | `packages/*`, `apps/*`, else `src/<dir>`; a boundary exists for any directory there, empty or not | The same, plus the members of a Cargo workspace (`[workspace] members`, plain paths and `dir/*`) and a root crate that is also a package; a directory with no owned file is no boundary | Rust-first scope: the first graph has to cover Rust. A glob the reader does not read (`**`, `?`) is a stated limit (`Boundaries::limits`). In the corpus: four Cargo workspaces (35 crates) replace the `src/*` guess; `pm2-eid-is-discord/src/types` (only `.d.ts` files) is no boundary. |
| Files a boundary owns | `ts tsx js jsx mjs cjs` | Also `mts cts vue svelte astro rs` | An `.mts` or `.svelte` import is a dependency like any other. Rust `src/a.rs` belongs to the boundary `src/a`. |
| Reading imports | A pattern over text (`from "x"`, `require("x")`, a line starting `import "x"`) | The syntax tree: static, `export ... from`, dynamic, `require`, `import x = require()`, `import('x').T`; the scripts of components; a computed specifier is kept and reported | The pattern counted imports in comments and strings (the only `zod` edge of `packages/tsc` was text in a benchmark), missed a statement over several lines when the keyword was not at the start, and could not see dynamic or type imports. |
| Resolving an import | A relative path is taken to name a directory by its prefix, even if no such file exists; a bare name matches a package name | A relative path must be a file: extensions in TypeScript's order, `.js` naming `.ts`, `index`, a directory's `package.json` entry, bundler `?query`; `tsconfig`/`jsconfig` `paths` and `baseUrl` through `extends`, falling through to the workspace package when a mapping misses; a bare name matches a workspace package by name | Corpus: `../router` is `router.ts`, not the directory `router/` (3 `hono` edges); `../styles/global.css` did not exist (1 `astro-preset` edge); `@/shared/x` follows `paths` (1 `ontara` edge). 5 edges the TypeScript found and this does not, each read. |
| Imports that resolve to nothing | Not recorded | Listed on the graph with a reason: `not-found`, `alias-target-not-found`, `unknown-alias` (`#x`, `~/x`, `$lib/x`, `@/x` with no mapping), `computed` | A review has to say what it could not follow. Corpus: 529 of them, mostly SvelteKit's `$app/*` virtual modules, `#` imports of a `package.json` and computed `import()`. |
| Rust edges | None | `declared` edges from `Cargo.toml` (`dependencies`, `dev-`, `build-`, `target.*`, renamed and path dependencies); `import` edges from `use`, `extern crate`, `mod x;` and paths in expressions, through each crate's module tree (`mod` files, `#[path]`, inline modules) | New: the TypeScript engine had no Rust graph. Declared edges equal `cargo metadata` on four corpus workspaces. |
| Layer of a boundary | A word of the vocabulary matched when the name equals it, contains `/word`, or ends with it; the first layer in order wins | A word must be a whole word of the name (split on anything that is not a letter or digit); a name with words of two layers is `unknown` | `ClientApp`, `HybridApp` and `WebApp` were application layers, `WebhookClient`, `Webhooks.API` and `WebAppComponents` presentation layers and `tui` presentation: seven corpus boundaries. Layers accuse other modules of violations, so a wrong layer is a false finding. |
| Layer violations and cycles | Over the JavaScript/TypeScript graph | The same rules over the new graph, so Rust paths count | Corpus: the TypeScript engine found 3 cycles and no violation; the 3 cycles are reproduced exactly (modules, shortest cycle, impact), and the Rust files add one violation (`psychoquine-cli`: `application` uses `infrastructure`) and one cycle (`orph-cli`: `cli`, `commands`, `tui`), each read. |
| Ownership | `CODEOWNERS` read at the root, `.github/`, `docs/`; the rule's pattern tested against the module's directory name | `.github/`, root, `docs/` (the platform's order); each file matched with the format's rules (no slash matches at any depth, `*` in one segment, `**` across, last match wins, an empty rule removes the owner); a module reports its files' owners and whether they agree | The module-id test only worked for rules that named the directory. Corpus: 147 of 147 modules agree. |
| Entrypoints of a module | `module` or `main` of its own `package.json` with `.js` read as `.ts`, else `src/index.ts`, `index.ts` | `exports` (every subpath, conditions in order, custom ones too), `types`, `module`, `main`, a build folder read as its source, `src/index` and `index` in any extension; a module inside a package gets the package manifest's entries inside it | Corpus: 112 more entries (zod, starlight, kit, hono), each named by a manifest. |
| Public API names | `export <declaration>` and one-line `export { } from` found by pattern | Every form of export from the syntax tree: destructuring, `abstract`, `let`/`var`, namespaces, `default`, `export { a }`, `export * from`, `export * as ns`, specifiers over several lines, an imported name exported again; names that start `type ` are not kept with their keyword | Corpus: 3,220 more names, each by one of those forms; 7 TypeScript names carried the `type` keyword. |
| Unexpectedly exposed | Every export of a non-entry file | Only what no entrypoint re-exports; not default exports; not the files a tool loads by name; not forwarded re-exports | Corpus: 3,726 fewer or different, each classified in `tests/corpus/module_api.rs`. |
| Dead code: what is a use | The name as a word in any scanned file (a `$` name never matches; the declaration counts once) | A name imported (or a namespace member read) and resolved to its declaration through barrels; own-file reads and uses by tests, scripts and components count; unresolved aliases protect a name | Corpus: 994 TypeScript findings not made, each explained; the word search missed 638 names that start with `$`. |
| Dead code: kinds | `exported-but-unused`, `probably-dead` | Plus `unreferenced` (a declaration nothing in its file reads, by scope); a name read only in its own file is not reported | Corpus: 4 unreferenced, 20 more unused names no file mentions. |
| Change impact | Affected modules, dependents, public names of changed files, three confidence levels | Same, plus the files no module owns, files the graph could not read and computed imports as reasons to lower confidence | A change outside every module was `low` only when none belonged. Corpus: 127 of 147 identical, 20 reach more because Rust and component files are read. |
| Public API of a Rust crate | Not read | What `pub` reaches from `lib.rs`, `pub use` followed, `unreachable_pub` as "exposed" | New. Against rustdoc: 381 of 381 items on two crates. |

## Defects carried over on purpose

The repository-intelligence port reproduces the TypeScript detectors, limits included, so that the differential tests can prove the port before anything is improved. These are known and are not parity decisions; replacing them is syntax-layer work (phase 3):

- **Tauri is detected when `Cargo.toml` contains the text `tauri` anywhere**, including a comment (the fixture `cargo-mentions-tauri-only-in-a-comment` shows a project that is not Tauri reported as Tauri and desktop). CLAUDE.md says a `Cargo.toml` is Tauri only if it depends on tauri; the code does not do that. The same applies to `flutter` in `pubspec.yaml`.
- **TOML values are read with a regular expression** (`tauri`, `rust-version`), so a key in any table matches.
- **A configuration file that is not JSON is skipped without a warning.** An `.eslintrc` in YAML contributes no criteria and the review says nothing about it; the `coverage` field of the contract is where this will be reported.
- **Documentation kinds come from substrings of the path**: any directory whose name contains `adr` makes its files ADRs.
- **Regular expressions run on ECMAScript semantics** (`regress`), not Rust's, because `\s`, `^`/`$` in multi-line mode and `.` differ (a title after a BOM, U+2028 or a lone `\r`).

## Collation

Rules, packs and conflicts are ordered with `String.prototype.localeCompare`, which is ICU's root collation, not byte order (`a` before `B`, `-` before digits, `é` beside `e`). The Rust engine uses ICU4X (`icu_collator`) and the test compares 3,481 string pairs against the TypeScript result; Node (ICU 78.3) and Bun (ICU 78.1) agree on all of them. The collation tables are Unicode data: an ICU upgrade that changes them would change the order of rules in a policy, and so the diff between two runs, but not the policy id (it depends only on pack references, sorted by id). The pairs are in `tests/fixtures/knowledge-conformance.json`; regenerate and compare when either side upgrades ICU.

## Known limits (not decisions)

- `Object.entries` of a configuration section that is a string iterates code points, JavaScript iterates UTF-16 units; they differ only for astral characters in a section that should be an object.
- A file or directory name that is not valid Unicode is shown with replacement characters, as Node does.
- The in-process context cache compares modification times as `f64` milliseconds; it is not persisted.
- A lone UTF-16 surrogate inside a string cannot be represented by `serde_json`; Node would hash it as U+FFFD. Real files do not contain one.
- Integers above 2^53 are rounded to a double before canonical hashing, as `JSON.parse` does.

## Bugs the differential tests found while porting

- `serde_json` parses long decimals imprecisely unless `float_roundtrip` is on; the snapshot hash would have differed for some numbers. Enabled in the workspace.
- Formatting numbers the way JavaScript does needs ECMAScript's own algorithm (`ryu-js`).

## Detector gate on the whole corpus

`crates/debuggatha-engine/tests/corpus/gate.rs` (opt-in, `just corpus-detectors`) runs every
detector together through `default_registry` and the golden policy of each of the 61 repositories,
and compares whole findings (everything but the random `id`; `appliedPolicy` is checked apart,
because the goldens were reviewed under another policy id than the one in `ts-context`) with
`corpus/golden/ts-findings`. Result, committed in `corpus/golden/divergences-all-detectors.json`: for the
sixteen rules that have findings, **0 findings differ in any field**, the findings both engines
report come out in the same order in every repository, and the only differences are findings one
side lacks: `no-unwrap-expect` 1230 and `no-panic` 34 only in the TypeScript (Rust test items),
`no-eval` 24 only in the TypeScript (PHP `->eval(` method calls), and the the JavaScript syntax rules differences. The
test fails if any difference is in no group golden, so a difference cannot hide here. The 9 golden
findings with no rule id are architecture findings of the analysis stage (phase 4), not detectors.
The gate also refuses a missing or empty clone: with `fastapi` empty (an interrupted fetch left it
so, and `scripts/parity/fetch-corpus.mjs` took it for cached; fixed) the Python secrets looked like 16
TypeScript-only findings. The the secrets rule count is now 132, not 116, all identical.

## `no-hardcoded-secrets`: false positives removed (after the first measurement)

`docs/DETECTOR_QUALITY.md` measured the rule at 48% on a 44-hit sample. Every change below makes the
Rust detector report **less** than the TypeScript one, never more, and each has a fixture where the
TypeScript reports and Rust does not (`tests/fixtures/group-e-conformance.json`, 17 "differs"
cases). The ledger fingerprint of a hit that is still reported does not change.

| Case | TypeScript | Rust |
|---|---|---|
| Interface text with accents or other alphabets (`'Confirmar Contraseña'`) | Word check read only `A-Z`: reported | Letters of any alphabet |
| A sentence of five or more words, with its punctuation | Reported when it had a comma or an accent | Not reported; three words (`'shhhh, very secret'`) stay candidates |
| Question marks and parentheses around a label (`'¿Olvidaste tu contraseña?'`, `'Contraseña (opcional)'`) | Reported | Separators, so words |
| A character class with a quantifier (`"([a-zA-Z0-9-]+)"`) | Reported | A regular expression, not reported |
| A value ending in a dot (`"base64url.bearer.phx."`) | Reported | A prefix to be completed, not reported |
| A value starting with `--` (`'--password='`) | Reported | A command-line flag |
| `required|string|current_password` | Reported | A list of validation rules |
| Password hashes (`$argon2`, `$2a$`/`$2b$`/`$2y$`, `$pbkdf2`, `$scrypt$`, `$5$`, `$6$`) | Reported | Not a credential |
| `'$prefix$id'` in Dart and Kotlin | Reported | Computed with interpolation, not hard-coded |
| A PEM header with no base64 body on the line or the next two | Reported, with high confidence | Code that writes a key is not a key |

On the 61 repositories the rule goes from 132 hits to 66. All 66 removed were read: 23 interface
texts in two Spanish dictionaries and 25 more strings of the same kind, 6 Argon2 hashes, 4 token or
regex constants, 2 key-header writers, 5 validation rules and the rest of the cases above; none was a
credential. Known limit: the fixes were found on this corpus, so the new figure is not independent
of it, and a Dart or Kotlin secret that contains `$name` is now missed (the price of the
interpolation rule). `looks_like_prose` needs five words, so a four-word UI label without
punctuation that the word check cannot read (digits inside) is still reported.

## `no-unwrap-expect`, `no-panic`, `avoid-print`: false positives removed

From the measurement in `docs/DETECTOR_QUALITY.md`. Each change reports **less** than the TypeScript
and has a fixture where the TypeScript reports and Rust does not (`group-c-conformance.json`, 21
"differs" cases).

| Case | TypeScript | Rust |
|---|---|---|
| `.expect(a, b)` with other than one argument (`self.expect(&kind, "why")`) | Reported | `Option::expect` and `Result::expect` take exactly one argument; a method of the program's own called `expect` is not one. A one-argument method of the program's own called `expect` is still reported. Inside a macro body the arguments are not counted (tokens only), so it is still reported there |
| `panic!` in a file named `build.rs` | Reported | A Cargo build script fails the build by panicking. Only the panic: an `unwrap` in it is still reported |
| `print(` in a Dart file under a `example/`, `examples/`, `tool/` or `bin/` directory | Reported | The program's output. `bin/` is the pub layout for executables and was not seen in the corpus; the other three were |

On the corpus: `no-unwrap-expect` drops 43 hits (all `self.expect(..)` with two or three arguments,
in `obfusku` and `syntaxeon`), `no-panic` drops 18 (all in one `build.rs`) and `avoid-print` drops 76
(all in `example`/`examples`/`tool` directories). Known costs: a Flutter application kept under
`examples/` is exempt from `avoid-print`; a `testutil.rs` helper is still reported by `no-panic`
(a path cannot tell it from code), and so are panics at start-up in Go.

## `no-panic` qualified macro path

The Rust `no-panic` hit now covers the whole path (`std::panic!`, from `std`), and only `panic!`, `std::panic!` and `core::panic!` are the standard macro: a `panic!` of another crate is not reported. The TypeScript engine reported any `x::panic!` and started the hit at the name.
| An added line whose content starts with `++` (it reads `+++ ...` in the diff), or a hunk line that looks like `@@ -1 +1 @@` | Read as content | The hunk header gives the number of lines, and exactly that many are consumed, so a content line is never a file header. The TypeScript parser drops such an added line or takes it for a new file |
| Diff paths | Names with spaces, git-quoted names (`\303\251`), renames, deleted files and binary files are read | The TypeScript parser reads `+++ b/path` with a regex: a quoted path or a name with a tab is wrong, and a rename has no added lines |
| `.debuggatha/baseline.json` with a field this version does not know | Refused | Same fail-closed reading as the ledger and the memory store: a save never drops what a read did not understand. The TypeScript engine ignores unknown fields |
| Order of baseline entries and key order in the file | By UTF-16 code unit; keys sorted | The TypeScript engine sorts entries with `localeCompare` and writes keys in insertion order. The files read the same way in both engines |
| Source line quoted in an analyzer finding | Known credential formats redacted, cut at 200 units | The TypeScript engine copies the trimmed line as it is; Rust gives it the same excerpt rule as its own findings |
| Why an analyzer failed (`AnalyzerRun.reason`) | Repository path replaced by `.`; a failure to start says only "could not start" | The TypeScript engine passes the tool's first line and the operating system's message on, which name paths |
| A tool path with `..` inside (`a/../../x`) | Dropped | The TypeScript engine drops only a leading `../` |
| Analyzers ported | All nine (Gitleaks, Biome, ESLint, Ruff, ktlint, detekt, PHPStan, Clippy, OSV-Scanner) | Each adapter's commands and parser are checked against output recorded from the real tool, copied from the TypeScript fixtures |
| Analyzers run in the process environment | Executables are searched in a list the caller hands over (`PATH` by default) | The TypeScript engine reads `process.env.PATH`; a list makes the search testable without changing the environment |
| Which files a model reads for detection | Any file the engine has a parser for (HTML excluded) | The TypeScript engine reads a fixed list: JavaScript, TypeScript, Python, Kotlin, Java, PHP, Rust, Go, Ruby, C#, Swift and Dart; C++, Elixir, GDScript and the Vue, Svelte and Astro files were left out |
| A detection's excerpt | Known credential formats redacted as well as long quoted tokens | The TypeScript engine only scrubs long quoted tokens |
| A model answer is read | Whole, not streamed (`stream: false`) | The TypeScript providers stream and join the pieces; the text is the same |
| A local model URL | `http://` to `localhost`, `127.0.0.1` or `[::1]`; no redirects, no environment proxy | The TypeScript engine also accepts `https://localhost`, follows redirects and uses the platform's `fetch` |
| An error reaches a client | The message is sent only for error types listed as free of machine details; a filesystem cause anywhere in the chain gets a fixed message and any other type gets "An unexpected error occurred." | The TypeScript engine relays any `Error` and replaces only those with an errno `code` |
| `review` with no scope | The working tree's changes in a git repository; with no changes, the whole repository | Same |
| `review --format json` | `{ status, policyId, summary, findings, coverage, analyzers, semantic, countedFindings, changes... }` | The TypeScript report nests the findings in `reviewResult` and adds `syncReport`; clients of the daemon read Protocol v1, not this |
| Text reports | No colors | The TypeScript CLI colors severities when writing to a terminal |
| `.debuggatha/config.json` | Read the same way, strictly; an error names the setting, never the machine path | The TypeScript loader validates with zod; the messages differ |
| A usage error on the command line | Exit code `1` | commander exits with `1` as well; clap's own default is `2`, which the CLI would confuse with findings |
| `findings reopen` | Moves the finding to `reopened` | The TypeScript command asks for `open`, which the lifecycle refuses from `resolved` and `dismissed`, so it only fails there |
| `--root` and `--format` | Global options, valid before or after the command | The TypeScript CLI uses `--cwd` and `--json` |
| `doctor` | Checks the version, git, the stored files (ledger, memory, baseline) and the context; a corrupt stored file fails it | The TypeScript command reports the Node version and only the context |
| `completion` | Scripts generated by clap for bash, zsh, fish and powershell | Hand-written scripts that complete command names and flags only |
| `mcp` transport | stdio and Streamable HTTP written for this engine, no MCP SDK; both tested against the TypeScript SDK's clients | Both through the TypeScript SDK | The engine is synchronous and threaded, and a server that only forwards to the daemon needs a few messages, not an async runtime. |
| `mcp --http` | Stateless, `POST /mcp` only, JSON responses (batches too); `Host` and `Origin` must be local; binds only to a loopback address (`--host 0.0.0.0` is an error); no model of the client, progress or cancellation; at most 64 connections; no chunked request bodies | Stateless, `POST /mcp`, the `Host` check; any `--host` | Nothing signs a caller in, and the tools write the ledger. The TypeScript server cannot sample over HTTP either. |
| `mcp` tools | The thirteen tools; their results are the daemon's (`findings` is a count, the first page is in `findingsPage`); no `outputSchema` | Same tools; results with `summary`, `findings`, `syncReport` and an `outputSchema` | The shapes follow Protocol v1 so one engine serves every client. |
| `mcp` repository | A call without `rootDir` works on `--root`, or, when none was given, on the first root the client shares (stdio), else the current directory. `rootDir` must be absolute and inside a root the person allowed (`--root`, `--allow-root`) or the client shares; anything else is refused without a path in the message. One daemon per repository | `rootDir` may be any path; no allow list | A tool call can write `.debuggatha/` files and read code, so which directories it can reach is the person's decision, not the call's. |
| `mcp` resources | The four `debuggatha://` resources and their `{?rootDir}` templates, under the same rules as a tool call; the ledger is `{ total, entries }` | The same four and their templates | The ledger's shape follows Protocol v1. |
| `mcp` model | The client's model through `sampling/createMessage` when the client can sample (stdio), or a local runtime chosen by `--semantic` | Sampling, or a configured local model | The same, and for a client of MCP 2026-07-28 (stdio or HTTP) that declares `sampling` in its per-request `_meta`: the call answers `resultType: "input_required"` with the prompts as `inputRequests` and a `requestState`, and the retry's `inputResponses` resume the review. Over HTTP this is the only way the client's model is reached. A call without the capability gets `-32021`. |

---

*Last updated: 2026-11-19*
