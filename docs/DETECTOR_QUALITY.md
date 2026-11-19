# Detector quality on the corpus

<!-- Measured false-positive rates of the ported detectors; for anyone changing or trusting a rule. -->

The quality bar below asks for a measured false-positive rate per detector, on the real
corpus, with the limits stated. Parity (`docs/PARITY.md`) proves the Rust detectors agree with the
TypeScript ones; it does not prove either is right. This is the first measurement of the ported
rules.

---

## Quality bar for a detector

A detector, query or analysis stage is done when it has:

1. A **positive corpus** (code that must be reported) and a **negative corpus** (look-alikes, strings, comments, generated code, test code, other frameworks that must not be reported), kept as fixtures.
2. A **measured false-positive rate** on the real-repository corpus (phase 0), recorded in the module README with the corpus commit ids. Medium-and-above confidence needs a target set per rule before merging; low confidence is allowed only when the limit is stated in the rule's `limitations`.
3. An honest **confidence**: `high` only when the syntax tree plus resolved facts make the claim certain; unresolved facts (an imported helper that cannot be followed, a value of unknown origin) produce "not verified", not a finding.
4. Its limits written in the pack knowledge entry and the module README.

---

**Method.** `DEBUGGATHA_SAMPLES=<file> DEBUGGATHA_CORPUS_DIR=<dir> cargo test -p debuggatha-engine
--test corpus --release` writes up to 50 findings per rule, evenly spaced over all of the
engine's findings in scan order (deterministic), with three lines of context. The corpus is the 61
repositories pinned in `corpus/manifest.json` (commits as of 2026-11-10). One reviewer (an AI
agent, not a person) read each sampled excerpt, and the surrounding code where the excerpt was not
enough, and judged two things: does the code do what the rule's wording says (**by wording**), and
would a maintainer agree it is a problem (**by intent**). Only the first is a false-positive rate in
the sense of the quality bar; the second is listed so the limit is not hidden.

**What this is not.** One reader, one pass, no second opinion, and the sample is spread over hits,
so a repository with many hits weighs more (`zod` for `no-explicit-any`, `promediatum` for secrets).
Nobody checked the false negatives: a hit that the detector missed is invisible here. The
thresholds of the bar (high 98%, medium 90%) are shown against each result, not applied as a gate.

| Rule | Confidence | Sampled / total | True by wording | Result |
|---|---|---|---|---|
| `no-hardcoded-secrets` | medium (some low, one high) | 44 / 132 | 21 | **48%, below the medium bar** (fixed, see below) |
| `no-unwrap-expect` | high | 48 / 424 | 43 | **90%, below the high bar** |
| `avoid-print` | high | 24 / 24 | 24 | 100% by wording; 22 by intent are scripts and examples |
| `no-panic` | high | 42 / 83 | 42 | 100% by wording; 12 by intent are build scripts and test helpers |
| `no-unsafe-blocks` | high | 33 / 66 | 33 | 100% |
| `no-eval` | high | 20 / 20 | 20 | 100% (PHP `eval`, JS `eval`) |
| `no-extend-native` | high | 6 / 6 | 6 | 100% |
| `eqeqeq` | high | 14 / 14 | 14 | 100% |
| `no-empty-tests` | high | 2 / 2 | 2 | 100% |
| `use-v-html-carefully` | high | 1 / 1 | 1 | one hit: no rate |
| `no-var` | high | 50 / 1986 | 50 | 100%; 2 are in bundled output (`priv/static`) |
| `no-explicit-any` | high | 50 / 2085 | 50 | 100% |
| `strict-null-checks` | medium | 49 / 781 | 49 | 100% |
| `prefer-object-spread` | medium | 31 / 31 | 31 | 100%; 3 test `__proto__` handling, where spread and `Object.assign` differ |
| `prefer-template-literals` | low | 48 / 707 | 48 | 100% by wording (a style rule) |
| `no-assertionless-tests` | low | 43 / 255 | not measured | see below |

Not in the table: `no-async-foreach`, `no-focused-tests`, `no-shell-interpolation`,
`no-open-cors`, `no-error-leak` and `async-route-errors` have no hit, or a handful, on this
corpus (`async-route-errors` 4, `no-error-leak` 1). Their proof is the fixtures; no rate can be
given.

## What the false positives are

- **`no-hardcoded-secrets`** (23 of 44). Interface text in Spanish and English assigned to a name
  that contains `password` (`confirmPassword: 'Confirmar Contraseña'`, 16 of them, in two
  repositories); a regular expression kept in a constant called `TOKEN`; a PEM header written by
  code that generates a key (`append("-----BEGIN RSA PRIVATE KEY-----")`, reported with **high**
  confidence); a token prefix; a Dart string with interpolation; a validation rule
  (`'password' => 'required|string|current_password'`); two Argon2 hashes. The 21 true ones are
  JWTs in tests and docs, a hex key and an example secret. The word check was written for English
  prose; accented words and short phrases pass it. This behaviour is the TypeScript's and is
  identical in Rust, so it is a defect to fix **with a listed divergence**, not a port error.
- **`no-unwrap-expect`** (5 of 48). `self.expect(...)` on a type's own method called `expect`
  (`obfusku` parser, `syntaxeon` checker), which is not `Option::expect`. A name is all the
  query can see: telling them apart needs the receiver's type.
- **`no-assertionless-tests`** (low). Of 12 read, none was clearly a test that verifies nothing:
  five delegate to a helper that asserts, four rely on an implicit assertion (`.parse()` throws,
  Playwright's `waitFor`), two are type-level tests. Low confidence is the honest label, and the
  helper resolution of phase 3 exists for this rule; the remaining 31 were not read.

## Decisions this leaves to the person

1. `no-hardcoded-secrets`: tighten the prose filter (and the PEM-header case) now, as a listed
   divergence with a before and after on this corpus, or leave it for the phase 3 work.
2. `no-unwrap-expect`: lower the confidence of a call named `expect` whose receiver is `self`,
   or leave it.
3. `avoid-print` and `no-panic`: exempt `example/`, `bin/`, `tool/` and `build.rs` paths, or leave
   them.

Nothing here was changed in the detectors: this was a measurement.

## After the secrets fix

`no-hardcoded-secrets` was changed (divergences listed in `docs/PARITY.md`) and re-read in full, not
sampled: 66 hits remain of 132, and the 66 removed were all false positives. Of the 66 that remain,
57 are JWTs in test code (reported at medium confidence, as a known format in a fixture), 4 are one
hex key in FastAPI's documentation, and 5 are one example secret, a Google API key, a base64
`username:password`, an all-zero `clientSecret` (low confidence) and a PEM example in a KDoc
comment. By wording that is 66 of 66; whether a test fixture should be reported at all is a policy
question, not a detector defect. **This is not an independent measurement**: the rules were written
looking at this corpus, and the next corpus may show other kinds of false positive.

`no-unwrap-expect` (90%), `avoid-print` and `no-panic` are unchanged.

## After the Rust and Dart fixes

- `no-unwrap-expect`: the 5 false positives were a program's own `expect(kind, what)` method; calls
  with other than one argument are no longer reported. 381 hits remain on the golden corpus. All 165
  `.expect(` calls among them pass a string, a constant or a `format!`, as `Option::expect` does;
  that is a check of the argument, not a reading of every hit. A one-argument method of the
  program's own called `expect` would still be a false positive; none was seen.
- `no-panic`: `build.rs` is exempt (18 hits in one repository). Panics in test helpers such as
  `testutil.rs` and at start-up in Go libraries remain, and remain arguable by intent.
- `avoid-print`: `example/`, `examples/`, `tool/` and `bin/` Dart files are exempt. On the golden
  corpus no `print` is left; the 8 left in the wider corpus are in library code.

Same caveat as before: written looking at this corpus, so not an independent measurement.

## `require-await` and `no-loop-func`

These two have no TypeScript twin, so there is nothing to be identical to: the proof is
`crates/debuggatha-engine/tests/detectors/functions.rs` (hand-written positives and look-alikes, 85 cases),
31 mutations that each fail a case, a seeded-recall test and a full reading of the corpus hits. The
corpus gate counts them apart (`newRules` in `corpus/golden/divergences-all-detectors.json`).

**`no-loop-func`.** First version: also reported a `let` declared outside the loop and written from
it. It found 16 hits, and all 16 were deliberate shared counters (`idx++`, `reads++`, a callback
that sets `connected = true`). The rule's own wording lets a function "exclusively reference
block-scoped variables", so the `let` branch was removed: it reports a `var` of the loop, or a `var`,
parameter or function declared outside it and written from the loop on. One hit is left on the
corpus (`starlight`, a parameter reassigned in a `while` and captured by a callback that is awaited
at once: true by the wording, safe by intent). **One hit is not a rate.** Modern code uses
`let`/`const`, so this corpus cannot measure the rule; the cases carry the proof.

**`require-await`.** First version: 105 hits. It now leaves alone what it cannot tell from an
interface that requires a Promise, without types: a function whose type comes from where it is
written (argument, object property, returned callback, typed variable, JSX attribute), one that
declares `: Promise<T>`, a generator, a stub, a method of a class with a parent, and test code.
That left **29 hits (15 high, 14 medium)**, all read. By the rule's wording all 29 are right (the
`await` that several of them contain is in a nested function, which does not count). By intent: one
is deliberate and says so in a comment (`(async () => task.fn())()` to turn a throw into a
rejection), and 11 are Next.js route, page and server-component functions, where `async` is the
convention and the framework accepts either. The rest are ordinary candidates. The cost of the
exemptions is recall: a callback that really did not need `async` is not reported.

**Closing the debt (2026-11-11).** Four items left open by the cut, each answered.

- **Precision of the medium hits.** I read the 31 medium hits outside PHP again (the scan now includes Ruby,
  Elixir and C++): 21 were variables of the project's own and 10 were not configuration someone must
  document: four Rust `env!` of names that a build script module prints (`bat`), and six variables a platform
  or tool owns (Windows' `PATHEXT`, Netlify's `CONTEXT`, `SST`, Flask's `FLASK_RUN_FROM_CLI`, Elixir's
  `RELEASE_NAME`, `PUPPETEER_CACHE_DIR`). That is 68% before. Changes: a `build.rs` that prints `cargo:rustc-env=NAME=..` documents `NAME`
  (`DocKind::RustSource`, only `build.rs` is read: the file listing skips `build/` directories, which hold
  output, so a script at `build/main.rs` is not seen and the four `bat` hits stay); the ambient list gained
  `PATHEXT`, `CONTEXT`, `SST`, the ten `RELEASE_*` names of Elixir releases, and the prefixes `FLASK_` and
  `PUPPETEER_`. Seven mutations of the new code each fail a case; look-alikes the project owns
  (`CONTEXT_ID`, `RELEASE_TOKEN`, `MYFLASK_X`) are still reported. **After: 25 medium hits outside PHP, 21 of
  the project's own and 4 build-time (84%), still under the 90% bar.** The fixes were found on this corpus,
  so the figure is not independent, and `CONTEXT` will hide a project's own variable of that name.
- **Scripts of Vue, Svelte and Astro components are read** (`syntax::script_regions` finds the blocks the
  parser already finds; each is read by the same oxc reader and its offsets moved back to the file). Cases:
  the three kinds of file, two scripts, an Astro frontmatter with a `return` before the read, JSX in a plain
  script, and the line and columns of a hit after markup and multibyte text; a script in another language,
  JSON, a style, markup text and a `<script>` inside a frontmatter string are not read. Six mutations
  (offset shift of start and of end, language of the block, frontmatter blanking, a foreign language) each
  fail a case; an eighth, the `return` option of the parser, decided nothing (oxc recovers and keeps the
  reads after it) and was removed. **The corpus cannot measure it:** its components read only the bundler's own
  variables (`DEV`, `MODE`, `PROD`, `BASE_URL`, 15 reads found by text search), which the rule leaves alone, so
  there is no hit and no rate.
- **Parse cost of the added languages.** 1,071 files, 6.3 MB (Ruby, Elixir, Swift, C++, GDScript, Astro) parse
  in 1.6 s on one core in a release build, about 4 MB/s, twice in a row; the 13,739 files of the parity list
  (66.8 MB) take 6.9 s. The added files are 9% of the bytes and about 23% of the time (C++ and Swift are the
  slow grammars). Parsing only; the detectors are not included. One machine.
- **The other direction of drift is not built.** I listed, in the 10 corpus repositories that carry a dotenv
  file, the documented names that appear in no other file of the repository: 37 in all. 34 are `bat`'s test
  fixtures for its own syntax highlighting, one is a `kit` test fixture, and the other two are
  `PHP_CLI_SERVER_WORKERS` (read by PHP itself) and `BROADCAST_CONNECTION` (read by Laravel). None is a stale
  entry the project would want removed, so the rule would be wrong in 37 of 37 reported. It would also need
  a finding on a file that is not a reviewed unit, and every source file of the repository to conclude
  "never read" (a diff review has not). Not built; reopen it if a real stale example shows up.

**Recall, seeded.** `tests/corpus/recall.rs` appends 12 known violations (8 forms of `require-await`,
4 of `no-loop-func`) to each of the 1,706 clean non-test JS/TS files of the corpus: 100% found, on
the appended line. A first run missed 5 and showed that two of my seeds redeclared a name the file
already had, which is invalid code, not a miss; the seeds now use unique names. This says nothing
about forms nobody thought of.

What this does not say: no second reader; the 29 judgments are mine. `require-await` at *high*
means "the tree shows no await", not "the `async` is a mistake".

### Review

A second pass over my own work found one defect and two wrong numbers.

- **Defect.** For a decorated method the hit landed on the decorator's line, not on the method's
  (`@Get('/x')` above `async m()`), which would have moved the finding's line, its ledger identity
  and the place an inline `debuggatha-ignore-next-line` has to go. Fixed (the report starts after the
  last decorator) with three cases.
- **Numbers.** The first write-up said "60+ cases" and "28 mutations"; the file has 85 cases and
  the suite is 31 mutations, all failing a case after the fix.
- **Independent check of precision.** Each of the 29 `require-await` hits was checked with the
  TypeScript compiler's parser: an `async` function starting on that line with no `await`, `for
  await` or `await using` of its own. 29 of 29.
- **Independent check of recall.** The same parser listed every `async` function in the corpus's
  non-test JS/TS files that has no `await` of its own: 158. 114 of them are exempt by one of the
  stated reasons (declared `Promise`, call argument, object property, stub, and so on), and the
  other 44 are exactly what the detector reports: 29 inside the gate's policies and 15 in
  repositories whose policy does not carry the rule (run on the files directly). No `async`
  function was missed or invented. That says the implementation matches the stated rule; it does not
  say the rule's exemptions are the right ones.
- **Cost.** A third oxc parse per JS/TS file (after groups A and B). The gate took about 13.1 s
  against 12.6 to 12.9 s before: not worth folding yet.
- **Not checked:** `no-loop-func` has one hit, so no oracle comparison was made for it; its proof is
  the cases, the mutations and the seeded recall.

## `no-floating-promises` and the module index

The rule is a typed one in its usual form ("is this expression a `Promise`?"), and the engine has
no types. So the detector reports a discarded statement only when the tree shows the value is a
promise, and `Coverage` says so: the rule is among the checked rules **and** has a rule gap of
reason `needs-type-information` (so a review that holds the rule is never `complete`). The proof is
`crates/debuggatha-engine/tests/detectors/promises.rs` (109 table cases, one more that runs the same files with
and without the index), 50 mutations that each fail a case, a seeded-recall test, a comparison
with the TypeScript type checker, and a reading of the corpus hits.

**What it reports.**

- *High*: the statement calls an `async` function (or one that declares `: Promise<T>`, or whose
  every `return` is `new Promise(..)` / `Promise.resolve(..)` and kin) of the same file, of a file
  of the review through the module index, or a method of the enclosing class (`this.m()`); or is
  `Promise.all|race|..`, `fetch`, `new Promise`, `import()`, an `async` function called on the spot,
  or a `.then`/`.finally` chain over one of those with no way to catch.
- *Low*: `x.then(cb)` with no rejection handler where nothing shows what `x` is. A thenable that is
  not a promise (a Cypress chain, a query builder) looks the same, which is why it is low.
- Not reported: `await`, `void`, `return`, assignment, argument, `.catch(f)`, `.then(f, g)`, and any
  call the tree cannot tie to a promise (a method of another object, a package, an alias).

**The module index** (`detect::modules`) answers one question: does this export return a promise?
It follows named, default, renamed and `export *` re-exports (eight hops, cycles end), resolves
relative paths with the extensions TypeScript tries, `index` files and `./x.js` for `x.ts`. It does
not read `tsconfig` paths or `package.json` exports, and in a diff review it only knows the changed
files. Both cost recall, not precision. It is built only when the policy holds a rule that reads it.

**Against the type checker.** I ran TypeScript's own checker (`allowJs`, no `node_modules`, so
packages are `any`) over every JS/TS file of the corpus and listed each discarded promise-like
statement that is not handled (2,171).

| | detector | the checker agrees |
|---|---|---|
| high | 60 | 60 |
| low | 81 | 52 |

The 29 lows the checker does not confirm were all read: `sleep(500).then` (from
`timers/promises`), `fastify.inject().then`, `this.login(..).then` (an async store action),
`window.showWarningMessage(..).then` (a VS Code `Thenable`), `nextTick().then`, jQuery's
`$.getJSON().then`. None is clearly not a promise; the checker just has no types for them without
the packages. So the low label is honest, and 52 of 81 is the confirmed floor, not a rate.

**What it does not see.** Of the 2,171 statements the checker finds, the detector reports 112
(60 + 52). The rest are calls on objects whose type only the checker knows: `reply.send()` and
`fastify.register()` (1,996 of them in one repository), `stream.close()`, `reader.cancel()`,
`get_value().refresh()`. That is the part that needs types; it is what the coverage gap states.
One form that did not need types was found by this comparison and is now covered:
`const delay = (t) => new Promise(..)` and `function sleep(t) { return new Promise(..) }`.

**Recall, seeded.** `tests/corpus/recall.rs` appends 10 forms to each of the 1,706 clean non-test
files: all found (1,706 of 1,706), except where the seed is not valid or not the global: the
`: Promise<void>` form is seeded only in `.ts`/`.tsx` (1,252), `fetch` only in files with no
`fetch` of their own (1,681). A first run missed 164 of the on-the-spot `async` call, which was an
ASI artifact of the seed (the file's last line had no `;`), not the detector.

**Corpus.** In the gate: 117 findings. Over all the corpus's JS/TS files: 141 (60 high, 81 low). The
first 38 high ones inside the gate (before the `sleep` form was added) were read before the comparison: `main()`, `run()`, `setup()` of
scripts and tests; `fetch(..).then(..)` in fastify's helper; `Promise.resolve().then(..)`; the
async actions of pinia's devtools. All true by the rule. Several are deliberate fire-and-forget
(hono's `run(stream, cb, onError)` has its own catch): the rule's knowledge entry says that
pattern exists, and `void` is the way to mark it.

**Decision taken: severity of a low hit.** A low hit (a thenable of unverified origin) is
reported at `medium`, not at the rule's `critical`, because the CLI's exit code follows severity
and a suspicion must not fail a run as a proven floating promise does. Only this detector does it
(`Hit.severity`); the runner and the other rules are untouched. High hits keep the rule's severity.
(I did not find the CLI's default `--fail-on`; at `medium` or lower a low hit can still fail a run.)

**Decisions this leaves to the person.**

- Test files are included. `.then` chains in tests are common, and some are deliberate; I did not exempt them because the rule applies there too.

**Limits.** One reader of the 38 highs and the 29 lows. `this.m()` assumes no subclass overrides
`m` with a non-promise. A local named `Promise` is not looked for inside the index (it is, in the
file under review). The comparison tests the implementation against the checker, not the rule
against what a person would want.

## `leak-mock-restore`

Read from the test file and, where the review holds it, the test runner's configuration
(`modules::TestConfig`, found by `restoreMocks`, `clearMocks`, `resetMocks`, `mockReset`,
`unstubGlobals`, `unstubEnvs` in `vitest.config`, `vite.config`, `jest.config` or the `jest` key of
`package.json`, applied to the files under that folder). The proof is
`crates/debuggatha-engine/tests/detectors/leaks.rs` (95 cases), about 75 mutations that each fail a case, a
seeded-recall test, a full reading of the corpus hits and an independent listing of every
creation site with the TypeScript parser. Every hit is *medium*: whether the override matters
depends on test order and isolation.

**What it reports**, in a test file: `vi.spyOn`, `jest.spyOn`, `sinon.spy|stub|mock`,
`vi.stubGlobal`, `vi.stubEnv`, and `process.env` written inside a function, when nothing puts it
back: `mockRestore`/`restore`/`mockClear`/`mockReset` on what it was kept in; a
`restoreAllMocks`/`clearAllMocks`/`resetAllMocks`/`sinon.restore()` anywhere in the file; any such
call in a hook; `using`; `unstubAllGlobals`/`unstubAllEnvs`; the same global or variable stubbed
again in an `after` hook; the same `process.env` key written or deleted in an `after` hook (or the
object replaced) or written twice in the same function; or the configuration. (The rule says
"restore or clear", so clearing counts.)

**What it leaves alone, and why.** A first version reported 64 hits; a reading of them gave three
families that are not leaks, each now a stated exemption:

- *The file's setup*: at the top of the file, in a `describe` callback, in `beforeAll`, in
  `vi.hoisted`. It is shared by design and the runner isolates files. (kit's file-level
  `vi.stubGlobal` constants were the largest group.)
- *An object the test makes*: a spy on a variable declared inside the test (`const app = new
  Hono()`), a parameter of it, or a variable that every `beforeEach` assigns a `new` object and
  nothing else assigns. Spying on it cannot reach another test. An alias of something outside
  (`const c = console`) does not count.
- *The reset itself*: a `vi.stubGlobal` in an `afterEach` is the put-back, not an override.

The cost is recall: a spy in a `beforeAll` or at the top of a file that does leak into the other
tests of that file is not reported.

**Corpus.** 16 findings in the gate: 7 in kit, 4 in starlight, 3 in hono, 1 each in astro-preset
and circe. All read, all leaks by the rule's wording: `process.env` set in a test and never put
back (astro-preset, circe, starlight's two), `vi.stubGlobal('fetch', ..)` in a test or helper
with nothing that unstubs it (kit, hono), `vi.spyOn(global.Intl, ..)` with no restore (starlight).
A real project may still not care (the kit `fetch` stubs are in a file whose other tests re-stub
other globals), which is what *medium* says.

**Independent check.** The TypeScript parser listed every creation in the corpus's test files: 256
sites. The detector reports 12 of them (the other 4 findings are `process.env` writes, which that
listing does not cover). Of the other 244, I read a random 45 and then every one with no restore
evidence anywhere in the file and not at the top level (20): all are explained by the exemptions
above or by the configuration (`animoria-vscode` sets `restoreMocks: true`, the only configuration
of the kind in the corpus). No missed leak was found. That reading is mine, and a sample.

**Recall, seeded.** `tests/corpus/recall.rs` appends 8 forms (inside `it` or `beforeEach`) to each
of the 1,706 clean files, read under a test file name: 1,706 of 1,706 for each.

**Limits.**

- A configuration the review does not hold (a diff review, a config outside the scanned files)
  cannot be read, so a project with `restoreMocks: true` would get false positives there. Coverage
  does not yet say so; there is no gap reason for it.
- `sinon` has no use in this corpus, so its forms are proven by cases only.
- `mockRestore` anywhere in a hook restores every spy of the file in the detector's eyes, which
  hides a spy that a different hook never restores.
- Cost: the index is now built for every policy with this rule (the testing pack applies
  everywhere), about 1.5 s more on the 13 s gate.

## `n-plus-one-queries`

The weakest of the phase 3 rules, by design. The syntax tree cannot say that `repo.find(id)` talks
to a database; that is a property of a type. The detector reads *shape and names*, so **every hit
is low confidence and is reported at `medium` severity** (the rule says `high`), the same decision
as for an unverified thenable in `no-floating-promises`. The proof is
`crates/debuggatha-engine/tests/detectors/n_plus_one.rs` (43 cases), about 25 mutations that each fail a
case, a seeded-recall test, a reading of the corpus hits and a listing of every `await` in a loop.

**What it reports.** A call that *looks like a query* inside a loop over a collection, with
arguments that use something declared inside that loop.

- *Query-shaped*: a receiver chain holding a data-layer name (`prisma`, `db`, `knex`, `sequelize`,
  `pool`, `connection`, `trx`, `manager`, `em`, `repo`, or a name ending in `Repository`, `Repo`,
  `Dao`, `Model`, `Db`) and a query verb (`query`, `findMany`, `find`, `count`, `create`, `update`,
  `where`, `first`, ...); or a class-like receiver (`User`) with a method only an ORM defines
  (`findOne`, `findById`, `countDocuments`, ...); or a template tagged `sql`.
- *Loop over a collection*: `for..of`, `for..in`, a `for` not bounded by a small literal, or the
  callback of `map`, `forEach`, `flatMap`, `reduce` and the like.

**What it leaves alone.** `while` and `do..while` (retries, polling, cursors); a loop over an array
written out; a `for` bounded by a number of ten or less; a query that does not use the iteration
(loop-invariant, a different problem); a function made inside the loop and run later; the query in
the head of the loop (it runs once); test files (seeding data in a loop is what a test does). The
rule's own knowledge entry names two of these (a loop over an array already in memory, a small
fixed loop).

**Corpus.** Two hits in the whole corpus, both in `bubble-catcher`'s `getChallenges`: for every
challenge, one query for the user's best submission and one for the count. That is a textbook N+1,
2 of 2, and **two hits is not a rate**: this corpus is mostly libraries and CLIs with no data
layer, so it cannot measure the rule. The TypeScript parser listed 162 `await`s in loops in
non-test files; every one was read by callee name. Besides those two, none is a database call
(they are `fetch`, file and stream reads, `reader.read`, hooks, parsing, Docker `container.stop`).

**Recall, seeded.** `tests/corpus/recall.rs` appends 6 forms to each of the 1,706 clean files:
1,706 of 1,706 for each.

**Limits.**

- A query hidden in a helper is not followed: `getRow(dialect)` in `bubble-catcher`'s
  `lifecycle.service.ts` runs a query and is awaited in a loop, and is not reported. Following
  same-file helpers would catch it; it is not done.
- The names are a list. A data layer called `store`, `gateway` or `client` is not recognised, and
  a receiver named `db` that is not a database is.
- A loop over an array already in memory, or over a handful of known keys, cannot be told from a
  loop over a growing collection (the rule's own limitation); only the literal and small-bound
  forms are exempt.
- Network calls in a loop (`fetch`) are the same shape of problem and are not covered: the rule
  says database.
- One reader; no recall or precision rate.

## `a03-injection` and `validate-input`

One small taint analysis (`detect::taint`) answers for both rules. The proof is
`crates/debuggatha-engine/tests/detectors/taint.rs` (14 groups, about 230 cases), about 100 mutations that each
fail a case, a seeded-recall test, a reading of every corpus hit and a comparison with the
TypeScript compiler's own listing.

**Sources.** `req.body|query|params|headers|cookies` where `req`/`request` is the first parameter
of a function; Koa `ctx.query`, `ctx.request.body`; `await request.json()`; Hono `c.req.json()`,
`c.req.query()`, `c.req.param()`. A destructuring taints every name it binds.

**Propagation** (one file, flow-insensitive, to a fixed point): initializers and assignments,
template literals, `+`, `||`/`??`/`?:`, member access, array and object literals, string methods,
`for..of`/`for..in` and the callbacks of `forEach`/`filter`/... over tainted data. Stopped by
numeric coercion, by `schema.parse`-style calls, and by **any call to a function the file does not
define** (its result is clean): that choice trades missed flows for no invented ones.

**`a03-injection` sinks.** SQL text built from tainted data (`query`/`execute`/`raw`... on a
data-layer or driver receiver, `$queryRawUnsafe`/`*Raw` anywhere, a template or concatenation in
`where`/`andWhere`); shell commands of `child_process`/`shelljs`; `$where` with tainted code; a
request object passed whole as a Mongo filter (`User.find(req.query)`). Safe forms left alone:
parameters, tagged `sql` templates, numeric coercion, a field of the body as a Mongo value
(`{ email: req.body.email }` is the idiom, not reported).

**`validate-input`.** A function whose first parameter is `req` (or `request` followed by
`res`/`response`/`reply`) that reads `req.body` or `req.query`, unless the handler calls a
validator on request data, the route is registered in the file behind a validator, or an
`app.use(validate...)` exists. Not reads: `!req.body`, `typeof`, `if (req.body)`, `req.body && ..`,
and arguments of `console`/`logger`/`log`. High confidence when the file registers the handler
and shows its whole chain; medium for a handler it does not register (a controller), since a
middleware elsewhere may validate.

**Confidence.** `a03-injection`: high when every link of the flow is a single binding, low (and
`medium` severity) when it passes through a variable assigned more than once, and always low for the
whole-filter Mongo form.

**Corpus.** `validate-input` 9 hits (7 Express examples, 2 fastify examples), `a03-injection` 0.
The 7 Express hits are handlers with no validation, correct by the rule's wording (one, the
`api-key` check in `web-service`, is arguable: it is authentication). The corpus golden does not
move: no repository policy of the corpus carries these rules, so the gate does not count them.
Against the TypeScript compiler: it lists 20 reads of `.body`/`.query` on a first parameter named
`req`/`request`; 14 are in handlers and all 14 are accounted for (7 Express hits, 2 fastify hits,
1 handler that only logs), the other 6 are Fetch `Request`s and are excluded on purpose. It lists
911 sink-shaped calls with a non-literal first argument; the 27 that mention a request and the 79
in files with a handler were read, and none is a query or a command fed by the request (maps,
`regex.exec`, context objects). **Zero true sinks means this corpus cannot measure `a03-injection`**:
it is mostly libraries and CLIs.

**Recall, seeded.** 14 forms appended to each of the 1,706 clean files: 1,706 of 1,706 for each.
The seeding found two real defects before this figure: a local `const require = createRequire(..)`
was not read as `require`, and names such as `schemaRoutes` or `invalidateCache` (and the body of a
middleware) counted as a validator before the route. Both are fixed and tested.

**Mutations.** 100 mutations; 26 survived the first round. Each became a case, or code that decided
nothing and was removed (`is_cleaner_name`, two branches of `call_taint`: an unknown call is
already clean). One survivor is equivalent: a validator *in the same argument as the handler*
(`validators[..=i]`) cannot be told from one before it, since function bodies are not searched.

**Limits.**

- No flow through a function the file does not define, and no stored-then-read data.
- Controllers imported from another file are reviewed when that file is reviewed (medium
  confidence); the module index of cut 2 is not used.
- Validation is recognised by imports from validation libraries and by words in names
  (`validate`, `sanitize`, `celebrate`): a validator named otherwise is missed, and a validation
  inside a service called with the raw body is reported at the handler.
- One validator call anywhere in the handler covers both `body` and `query`.
- LDAP, XPath and template-engine injection are not covered.
- Fixes were found on this corpus, so the figures are not independent. One reader.

## `document-env-vars`

Environment drift: a variable the code reads and no file of the repository mentions. The rule is new
(pack `debuggatha/dx`, now 1.1.0; the pack is `always`, so **every** policy id changes and the TypeScript
fixtures and the 61 context goldens were regenerated). It is **language-agnostic by construction**: two
halves that do not know each other, plus a new input. The proof is `crates/debuggatha-engine/tests/detectors/env.rs`
(25 groups), about 110 mutations that each fail a case, a seeded-recall test and two independent listings.

**Input.** `run_review_with_files(policy, units, Some(repo_files), ..)`; `RepoFile { path, content }`.
`scan_scope::list_repo_files(root)` lists them (hidden directories such as `.github` and `.cargo` count,
dependency and build directories do not; at most 1 MiB each). `run_review` keeps its signature and
passes `None`: the rule then reports **nothing** and `Coverage` lists it under the new gap reason
`needs-repository-files` (added to both schemas and `docs/schemas/README.md`; an additive value in an enum
that clients read). `Some(&[])` means "there are none": every read is undocumented. Nothing in the
workspace calls the runner yet (the CLI is phase 5), so nobody passes the files yet.

**What documents a variable** (`detect::env_docs`, names only, never values). `.env*` example files and
`example.env`: `NAME=` at the start of a line, also behind `#`. Markdown, reStructuredText and AsciiDoc: names
written in code (a code span or fence), as a bare upper-case word. Dockerfile and Containerfile: `ENV`, `ARG`
and `$NAME`. Compose and CI workflows, `Makefile`/`*.mk`/`Justfile`/`Taskfile`, and the manifests of package
managers and platforms (`package.json`, `composer.json`, `pyproject.toml`, `Pipfile`, `tox.ini`,
`.cargo/config.toml`, `Procfile`, `wrangler.*`, `vercel.json`, `netlify.toml`, `fly.toml`, `render.yaml`,
`mix.exs`, `pubspec.yaml`...): `NAME=`, `NAME:`, `NAME ?=`, `${NAME}`, a list item `- NAME`. A key counts only in
upper case (`run:` is a recipe). No YAML or TOML parser: these files are read for the names they mention, and
the cost is that any upper-case key or assignment in them counts as a mention.

**What reads a variable** (`detect::env`). JavaScript and TypeScript (oxc): `process.env.X`, `process.env['X']`,
`import.meta.env.X`, `Bun.env.X`, `Deno.env.get('X')`, `const { X } = process.env`. Rust, Python, Go, PHP, Java,
Kotlin, C#, Ruby, Swift, Dart, Elixir, C++ and GDScript: a tree-sitter query (`data/queries/env-*.scm`) finds a
call or index whose first argument is a string literal, and a table says which callees read the environment.
A name that is not a literal, an interpolated one, and a write (`os.environ['X'] = ..`, `process.env.X = ..`,
`delete`) are left alone; Rust `env!` is read inside other macros and attributes too.

**What it reports.** The first read of each undocumented variable in a file, one hit per line. Medium
confidence (the file cannot know a wiki or a deployment repository documents it). A read that handles the
variable being absent (`|| 'x'`, `?? 'x'`, `os.getenv('X', 'x')`, `ENV.fetch('X', 'x')`, Rust
`.unwrap_or(..)`/`.ok()`, `option_env!`, Go `LookupEnv`, Dart `defaultValue:`, PHP `?:`/`??`) is **low confidence
and `low` severity**: the program starts without it. Left alone: test files; the variables a platform or tool
owns (`AMBIENT`: `PATH`, `HOME`, `CI`, `NODE_ENV`, `GITHUB_*`, `CARGO_*`, `VERCEL_*`, `OTEL_*`, proxies, ...)
and, on `import.meta.env`, the bundler's `MODE`, `DEV`, `PROD`, `SSR`, `BASE_URL`.

**Corpus** (4,738 documentation files read; 1,006 reads). 339 hits: 207 low, 132 medium. Two independent
listings agree with the detector: the TypeScript compiler finds 313 JS/TS reads, the detector the same
313 minus the 5 bundler built-ins it leaves alone, and none the detector has and the compiler does not; a
text search finds the same reads for Go, Python, PHP, Java, C# and Dart (the differences it showed: a
docstring and a Blade `@env('staging')` that are not code, multi-line calls its line-based search missed).
It showed one real miss, `env!` nested in `include_str!(..)`, `concat!(..)` and `#[command(..)]` (28 reads,
fixed). I read the 28 medium hits outside PHP: **22 are variables of the project's own** (`FORCE_REBUILD`,
`POLLUX_CORE_DIR`, `NEXT_PUBLIC_INKEEP_KEY`, `WEBHOOK_URL`...) and 6 are standards or another tool's
(`OTEL_*`, Netlify's `CONTEXT`, `SST`, `PUPPETEER_CACHE_DIR`, `FLASK_RUN_FROM_CLI`, `SVELTEKIT_FORK`); `OTEL_`
was then added to the ambient list. That is about 80%, **under the 90% bar for medium**, and the 104 medium PHP
hits are Laravel `config/*.php` keys read as `env('KEY')` (PHP returns null: absent is not an error) that
`.env.example` files usually leave out. The corpus has no application in Ruby, Elixir, Swift, C++ or GDScript
with an environment read, so those five are proven by cases and seeded recall only.

**Recall, seeded.** 26 forms over 13 groups of languages appended to every clean non-test file of that
language (JS/TS 1,731 files each; Rust 800; PHP 1,837; Python 737; Dart 553; C# 489; Kotlin 498; ...):
100% in every form. Seeds were skipped, as artifacts, for PHP templates that end in HTML and Ruby files with
`__END__`.

**Mutations.** 110; 36 survived the first round. Each became a case or code that decided nothing and was
removed (the `touches_error` check, which `make_hit` already applies; the oxc `fatal_error` return; a `::`
test that the next check implies; a final sort; a `pairs` counter).

**Limits.**

- **The TypeScript scan never delivered Ruby, Elixir, Swift, C++ or GDScript files**: its list is the parity
  list (`PARITY_EXTENSIONS`), which the gates keep. A review of a workspace uses `SCAN_EXTENSIONS`, which adds
  those five and Astro (listed in `docs/PARITY.md`); files named on purpose were always read.
- The templates and styles of single-file components, and HTML, are not read (their scripts are, see
  "Closing the debt" below); coverage lists that as a `no-detector` detail on the rule.
- Only one direction: a variable read and not documented. The other half of drift, a documented variable
  the code never reads, is measured below and deliberately not built.
- A name that is built (`getenv(prefix + name)`), a framework reading its own variables from a declaration
  (`#[arg(env = "X")]`, a `.env` loader), and values read through a wrapper (`config.get('X')`) are not seen.
- In a diff review the files are whatever the caller passes; a changed `.env.example` that is not passed
  leaves new variables looking undocumented.
- "Mentioned" is not "explained": a name in a README code span counts, with no check that the text says
  what it is for.
- No Intel darwin, no CLI wiring. One reader; the corpus is mostly libraries, so the figure for the medium
  hits is small and not independent of the fixes it motivated.

## Concept rules over the semantic model

Measured on the 61-repository corpus, non-JavaScript files only, `no-eval`, `no-panic`,
`no-shell-interpolation`, `n-plus-one-queries` and `a03-injection` read from the semantic model
(`data/concepts.toml`, `data/semantic/*.scm`).

- Reported: `no-panic` 34 Rust and 32 Go (unchanged rule), `no-eval` 24 (Python 4, PHP 18, Ruby 1,
  Elixir 1), `no-shell-interpolation` 1. `n-plus-one-queries` and `a03-injection`: none.
- Read by hand: the Python, PHP, Ruby and Elixir `no-eval` hits are string evaluation. Two false
  classes were found and removed: Ruby `class_eval`/`instance_eval` with a block (14), and PHP
  `exec` built only from fixed text and `escapeshellarg` parts (3).
- **Recall is not measured on real code.** The corpus holds no vulnerable query in these languages:
  the Flask tutorial and the Rust `conn.execute` calls bind their parameters. The cases that
  specify the rules are in `tests/detectors/concepts.rs`, nine to ten languages each.
- Not checked: Swift, C++ and GDScript have calls but no names, so `n-plus-one-queries` and
  `a03-injection` do not run there; tables were written from the languages' common libraries and
  miss any wrapper or ORM they do not list.

## Rules for languages without a pack

Eight rules over the concept tables, with the source each one cites: `no-unsafe-deserialization`
(CWE-502: Python `pickle`, `marshal`, `yaml.unsafe_load`; PHP `unserialize`; Java and Kotlin
`readObject`; C# `*Formatter.Deserialize`; Ruby `Marshal`), `no-weak-hash` (CWE-327), `no-print-stack-trace`,
`no-fatal-error` (Swift), `no-raw-markup` (CWE-79, Elixir and Ruby), `no-permit-all` (CWE-915, Ruby),
`no-atom-from-input` (Elixir, only when input reaches it) and `no-unbounded-c-string` (CWE-120, C++).

- Corpus (61 repositories): `no-fatal-error` 15 (Alamofire), `no-print-stack-trace` 11,
  `no-unsafe-deserialization` 34, `no-weak-hash` 4; the other four, none.
- Read by hand: every hit is the call the rule names. Two classes were removed: PHP `md5`/`sha1`
  (cache keys, not security uses) and `unserialize` with an `allowed_classes` option, which limits
  the call. `no-weak-hash` stays at low confidence because the use of the hash is not visible.
- Not covered, because they need an argument's value or a type: `getInstance("MD5")`,
  `verify=False`, `shell=True`, `UserDefaults` secrets, `.Result`/`.Wait()`, `try!` and `!` in Swift,
  `catch` with an empty body, `async void`, a mutable default argument, a raw `new`/`delete`.
- Ruby calls without arguments (`x.html_safe`, `params.permit!`) are now calls in the model.

## Balance between languages

`tests/packs.rs` counts, for each language, the rules of the shipped packs that a detector can report there, and fails when a language has fewer than half of what TypeScript has. Rules checked per language now: TypeScript 27; Python 29; Csharp 24; Kotlin and Ruby 23; Java 22; PHP 21; Go 20; C++ 19; Elixir 18; Dart and Swift 17; Rust 16. Before, TypeScript had 25 and the others between 2 and 9.

What raised them:
- **Rules written once over concepts and bound per language** (`data/concepts.toml`): debug output, weak random numbers, weak hashes and ciphers, SSRF, path traversal, open redirect, template injection, unsafe reflection, regular expressions and XML parsers built from input, process exit, type punning, unsafe deserialization, and an empty error handler (a capture of every adapter).
- **Rules of one construct written as a pattern** (`data/patterns/*.scm`, the capture `@hit.<rule>` is the code to report): Python `assert`, bare `except`, mutable default arguments and `verify=False`; Java wildcard imports and `finalize`; C# `async void` and `lock (this)`; Go `unsafe`, `pprof`, `defer` in a loop and `InsecureSkipVerify`; Rust `static mut`; PHP `@`; Kotlin `!!`; Swift `try!`, `as!` and `!`; Dart `dynamic`, `!` and `as`; C++ `new`, `delete`, `malloc`, casts and `catch (...)`.
- **Rules added to the packs** where a detector already existed (the shell, deserialization and hash rules in the packs of Go, Kotlin, Swift, Dart, PHP and others). A rule that exists in no pack never runs in a policy.
- **Test files of more languages** are recognized (`test_*.py`, `*_spec.rb`, `*_test.exs`, `*_test.cc`) and a suffix such as `Commit.java` is no longer taken for a test (`docs/PARITY.md`).

Frameworks outside JavaScript now have rules that read their code, and are applied when the manifest names the framework: Django 5 (CSRF exemption, debug, secret key in code, host wildcard, insecure cookies), Flask 1, Spring 3 (CSRF disabled, `permitAll` for every request, field injection), Rails 1, Phoenix 2 (SQL fragment interpolation, origin check off) and ASP.NET Core 4 (with the one it had); `no-open-cors` serves all of them and Express. The JavaScript frameworks have about 66 rules, almost all without a detector.

Not yet: Python is the only language above TypeScript; Java, C#, Ruby and PHP, older than TypeScript, are still 3 to 6 rules behind it. What they lack are rules that need the types or the values of arguments, and the rules of their frameworks (Spring, ASP.NET, Rails, Django, Laravel and Phoenix have 0 to 5 rules and almost no detectors). The corpus gives the noisy rules: `no-debug-output` 190 and `no-swallowed-error` 132 outside example, script and command directories, `no-force-cast` 381, `no-force-unwrap` 340, `no-double-bang` 258, `avoid-dynamic` 257, `no-assert-in-production` 168 and `no-catch-all` 99. Each rule's `limitations` name its known false classes.

## JavaScript over the model

The oxc adapter (`semantic/oxc.rs`) fills the same relations as the queries. `n-plus-one-queries` now runs over
it for JavaScript and TypeScript as for every other language, and the old oxc detector is gone: the 43 cases that
specified it pass unchanged, and on the corpus the old and the new report the same two lines. Two things the model
needed for that: loops that run a fixed small number of times (a list written out, a `for` bounded by ten or less)
and loops that stop at a function boundary.

The same cases were run against `a03-injection` over the model and fail on what the model does not carry: NoSQL
filters, `$where`, query-builder `where` with a template, spawn arguments, the expressions that are not data
(comparisons, `Array.isArray`, a length), destructuring of the request and the second-pass links. The request-shaped
taint detector stays as the JavaScript enrichment of that rule, as do `no-eval` and `no-shell-interpolation`
with their TypeScript parity (the concept table still binds both for every other language).

- `no-eval` and `no-shell-interpolation` over the model, against their 107 and 149 cases: 9 and 2 differ. `setTimeout` and `setInterval` with a string, and a shadowed `setTimeout`, are not calls the concept can name (the argument is text, and the callee must be the global); `new Function` is in the concept and not in the JavaScript rule; the JavaScript rule reports in test files and the concept skips them; the old rule reports `exec("a" + "b")` and not `exec(name + "x")`, the opposite of what flow says. They stay as JavaScript detectors.
- What stays on oxc is a rule about a feature of the language (`require-await`, `no-floating-promises`, `no-loop-func`, `leak-mock-restore`, `eqeqeq`), as `no-unwrap-expect` is for Rust. A rule about a concept every language has goes through the model.

---

*Last updated: 2026-11-19*
