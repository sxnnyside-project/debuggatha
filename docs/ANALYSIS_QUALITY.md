# Analysis engine quality on the corpus (phase 4)

The analysis stages **discover**, they do not judge (`CLAUDE.md`). Their quality is whether the model
they build is right, so this file records what each stage was compared with, not a false-positive
rate. Every difference with the TypeScript engine is in `docs/PARITY.md`; `tests/corpus_graph.rs`
fails on any difference nobody explained.

## Module boundaries, import resolution, dependency graph

Code: `crates/debuggatha-engine/src/analysis/` (`workspace`, `boundaries`, `imports`, `jsonc`, `resolve`,
`rust`, `graph`). Every stage is a function of a `Workspace` (paths and the content of sources and
manifests), so it is tested with a few lines of text. Scope decided with the person: JavaScript/TypeScript
(oxc) and Rust (tree-sitter) first; every other language is not covered and the coverage report must say so.

**Proof.**

- 46 hand-written cases in `tests/boundaries_and_graph.rs` and `tests/rust_module_tree.rs` (every import form, text that only looks
  like an import, computed specifiers, the lines of imports in components, TypeScript's extension and `index`
  order, `paths`/`baseUrl`/`extends` with comments in the JSON, a mapping that misses, workspace package names,
  Cargo members and globs, a root crate, `mod` files, `#[path]`, inline modules, `super`/`self`/`crate`, group
  `use`, expression paths, crates of the workspace, files no module reaches).
- 37 mutations of the code, each failing at least one case; 11 survived the first round: 3 were code that decided
  nothing (a `quasis.len()` test that is always 1, a length sort that no input can tell apart, a recursion depth
  already covered by the cycle guard) and were removed, and 8 became cases.
- **Against the TypeScript engine** (`scripts/parity/dump-analysis.ts` dumps its boundaries and graph;
  `tests/corpus_graph.rs` compares): 137 edges identical; 5 the TypeScript found and this does not (all read,
  all in `PARITY.md`); every edge only this one finds comes from Rust, a component, an `.mts` file or an alias.
- **Against `cargo metadata --no-deps`** (an independent oracle for declared edges): `ripgrep` 17/17, `obfusku`
  20/20, `syntaxeon` 27/27, `clap` 9/9. The first run missed the edges of a root crate that is also a package
  (`ripgrep`, `clap`); it became a boundary.
- **Against a text search for `crate_name::`** (an oracle for Rust import edges): `obfusku` 16/16, `syntaxeon` 26/26;
  `ripgrep` has 7 more (`pub extern crate x as y` and `use` in test modules, which the search missed; read);
  `clap` has 1 fewer (`clap_derive` mentions `clap::` only inside `quote!` bodies, which are tokens).

**What it does not do.**

- 529 imports are left unresolved on the corpus (listed with a reason, on `DependencyGraph::unresolved`).
- Rust: 178 of 1,019 files are in no module tree. 173 are `clap`'s `tests/testsuite/*`, declared by the macro
  `automod::dir!`; the rest are fixtures. They are listed in `DependencyGraph::unrooted_files` and their paths
  are not resolved. Macro bodies are tokens, so a path written inside `println!` or `quote!` is not seen. `cfg`
  is ignored (every `mod` counts). `use a::*` names the module `a`, not the items.
- JavaScript: `package.json` `exports` and `imports` (`#x`), bundler aliases (`vite.config`), and anything in
  `node_modules` (never scanned) are not read. A `tsconfig` that extends a package is not followed.
  A shadowed `require` is still read as `require`.
- Boundaries: Cargo globs other than `dir/*` are a stated limit; a root-level file of a split crate
  (`src/lib.rs` beside `src/<dir>` boundaries) belongs to no boundary, so its edges are not drawn.
- No dependency between languages (a Rust crate and the npm package that wraps it).
- One reader judged the 5 TypeScript-only edges and the Rust import extras. The corpus is 61 repositories, 10 of them
  with a Rust workspace.

## Layers, upward dependencies, cycles

Code: `analysis/layers.rs`, `analysis/cycles.rs`. A boundary is placed in a layer from the words of its own
name (never from its dependencies); an upward edge (domain, application, adapter, infrastructure,
presentation: an inner layer using an outer one) is a violation; a cycle is a strongly connected component of
the graph (Tarjan, iterative: 30,000 modules in one loop do not overflow the stack), with the shortest way
round and an impact by size (2 low, 3 medium, 4 or more high).

**Proof.** 14 hand cases (`tests/layers_and_cycles.rs`): 35 names for the layer model, every direction of the
layer order, unknown ends, merged evidence and the explanation text, cycles of 2, 3 and 4 modules, overlapping
and disjoint loops, a node leading into or out of a cycle, an edge into a finished component, the order of the
result. 19 mutations: 4 survived the first round (a same-layer edge, the sort of the modules, the on-stack test
and the lack of a case that enters the search out of order) and became cases; 2 more cannot change the answer
(the search stays inside the component and never revisits a module; they bound its cost and say so in a
comment). Against the TypeScript engine on the corpus: the 3 cycles are identical, it found no violation,
and 7 layers differ because of the whole-word rule (`docs/PARITY.md`).

**New on the corpus, read by hand.** `psychoquine-cli`: `src/application` uses `crate::infrastructure::detector::SystemDetector`
(a real upward dependency by the convention). `orph-cli`: `cli` names `crate::commands::*::Args` types and
`commands` uses `crate::cli::OutputFlags`; `tui` joins through `commands`.

**Limits.**

- The vocabulary is the TypeScript engine's (five layers, 28 words) and the order is a convention, not a
  law: an `adapter` using `infrastructure` is flagged because the order says so. 34 of 151 corpus boundaries get a
  layer (now 27); the rest are `unknown` and accuse nobody.
- Rust crates whose names carry a layer word (`obfusku-core` is a domain) are placed; nothing else about a crate is.
- A cycle is reported once per component. Which edge to cut is not said.
- Evidence of an upward edge is the import lines of the graph; a violation is not tied to a line of its own.
- Single reader, 61 repositories, 3 TypeScript cycles and 1 violation to compare with.

## Ownership, public API, dead code, change impact

Code: `analysis/ownership.rs`, `analysis/exports.rs` (what a file exports, imports and leaves unread, from oxc
and its scopes), `analysis/public_api.rs` (JavaScript and TypeScript), `analysis/rust_api.rs` (Rust),
`analysis/dead_code.rs`, `analysis/change_impact.rs`.

- **Ownership** reads `CODEOWNERS` (`.github/`, root, `docs/`, in that order) with the file format's own
  matching, per file: a pattern with no slash matches at any depth, `*` stays in one segment, `**` crosses,
  the last match wins, a rule with no owner takes ownership away. A module reports the owners of its files and
  says when they differ; a module with no analyzed file (a Dart package) is owned as its directory is.
- **Public API (JavaScript/TypeScript).** Entrypoints come from `exports` (every subpath, conditions in the order
  `types`, `import`, `module`, `default`, `require`, then any custom one such as `@zod/source`), else `types`,
  `module`, `main`; a build path (`dist/index.js`) is read as its source; a module inside a package gets the entries
  the package's manifest names in it. Re-exports are followed to where a name is declared, `export *` included. A
  name exported by another file of the module and re-exported by no entrypoint is "unexpectedly exposed".
  A file a tool loads by name (config, test, route, hook) exposes nothing, and neither does a default export.
- **Public API (Rust).** What `pub` reaches from `lib.rs`: `pub` items through `pub` modules, `pub use` (alias, list,
  glob, `self`, `super`, `crate`) followed to the declaration, a re-exported module bringing its `pub` contents,
  `#[macro_export]` at the root, `#[doc(hidden)]` as unstable, `#[cfg(test)]` skipped. A `pub` item no `pub` path
  reaches is "unexpectedly exposed" (what `unreachable_pub` says).
- **Dead code (JavaScript/TypeScript only; Rust is the compiler's `dead_code`, cut 5).** `unreferenced`: a top-level
  declaration nothing in its file reads, by scope (a parameter of the same name is not a read; a script file, a
  decorated class, `_name`, ambient code and single-file components are never reported). `exported-but-unused`: a name an
  entrypoint publishes that no file imports. `probably-dead`: a name of a non-entrypoint file that nothing imports
  or reads. A use is an import by name resolved to the declaration through barrels; `import()`, `require()`, a
  namespace passed on or read with a computed key, and a path into a package that no manifest entry or file names
  count as a use of everything they could reach; a name imported through an alias the analysis does not read is
  never called dead. A name read in its own file is not dead (the surface report says its `export` reaches nobody).
- **Change impact** maps files to modules, finds dependents through the graph (transitively), names the public names
  declared in the changed files, and lists files no module owns. Confidence falls to `medium` for an unmapped file,
  a file the graph could not read, computed imports, or a public name, and to `low` when no file belongs to a module.

**Proof.** 63 hand cases (`tests/module_api.rs`). Two mutation rounds, 64 mutations, each fails a case (the script tells a failing case from a compile error). Against the
TypeScript engine on the 61 repositories (`tests/corpus_module_api.rs`), every difference needs a rule that
explains it and the test fails otherwise:

| Stage | Same | Different, explained | Unexplained |
|---|---|---|---|
| Ownership (147 modules) | 147 | 0 | 0 |
| Entrypoints (modules with a surface) | 57 | 112 extra entries (`exports` subpaths, build paths read as source) | 0 |
| Exported names | 54 | 3,220 (2,589 through `export *`, 388 in entries only this engine finds, forms the line pattern misses) | 0 |
| Exposed names | 2,451 | 3,726 (re-exported by an entrypoint 1,734, test/config/route files 1,196, forwarded re-exports, `export *` namespaces) | 0 |
| Dead code the TypeScript engine reports | 51 | 994 (638 names that start with `$`, which its word boundary never matches; read in own file; tests and components it did not read; namespaces and dynamic imports counted as a use) | 0 |
| Dead code only this engine reports | 0 | 213 (see below) | 0 |
| Change impact of each module (147) | 127 | 20 (Rust and component files, cut 1 edges) | 0 |

The Rust surface was compared with the item list rustdoc writes (`cargo doc`): `debuggatha-core` 199 of 199 items,
`debuggatha-engine` 182 of 182, nothing missing and nothing extra (opt-in test `rust_surface_against_rustdoc`).

**What the corpus found in the first versions** (all fixed, each now a case): an import of `@x/ui/tokens.css` marked
every file of the package used; a `dynamic import()` of a package marked every file used; `export { a }` of an
imported name was a declaration; a custom export condition was needed to reach `zod`'s source; `exports` listing
only build output needed the usual source place; default exports of config files and SvelteKit/Next route files
were flagged; names behind `#app/...` aliases were called dead (kit).

**The 213 findings only this engine makes.** 4 are unreferenced declarations (the TypeScript engine looked only at
exports), 20 have no other mention anywhere, 4 are the same symbol under another kind, and 185 have the name mentioned
in some other file. Before the last fixes I read 14 of an earlier list of 246 of those at random: 11 were right (a
different symbol with the same name, a re-export no file imports, a use only inside the declaring file), 3 were false
positives of the first version (names imported through `#app/...` aliases, Next route files) and were fixed with
cases. The 185 that remain were not re-read.

**Limits.**

- Dead code reads JavaScript, TypeScript and the scripts of Vue, Svelte and Astro; the template of a component is not
  read, so its own declarations are not reported and a namespace it imports counts as used. Rust has a surface and no
  dead code. No other language has either; a report must say so (`covered: false`).
- Consumers outside the repository are invisible: a name of a publishable package is at most `low`.
- A namespace that escapes, a dynamic import, an `export * as ns` and a deep import all count as uses, so recall drops
  where code is written that way (zod: 638 names the TypeScript engine reports sit behind such a use or start with `$`).
- Rust: the associated items of a type (methods, fields, variants) and impl blocks are not listed; `cfg` is not evaluated;
  glob re-exports of another crate are not expanded. Package `imports` (`#app/x`) and bundler aliases are not resolved
  (cut 1), so names behind them are protected, not tracked.
- A file the project loads by convention is recognised by its name and folder (`+page`, `*.config.*`, `pages/`, `routes/`,
  `app/page|route|layout`, `hooks.*`, tests): a framework outside that list will show noise.
- Change impact from a list of files is file-level: any public name declared in a changed file counts. From a diff it also names the exports whose statement sits on a touched line; the declaration line of a re-exported name is not tracked, so a change inside a declaration far from its export statement is only known to be in a changed file. A removed file is not in the graph (read from the tree as it is), so what imported it is not followed and the estimate is never `high`.
- Single reader; the 185 are not re-read; the corpus has one CODEOWNERS style.

## Modules and edges outside npm and Cargo

Measured on the corpus with `infer_boundaries` and `build_graph`: gin 7 modules and 12 edges, fastapi 5 and 4, eShop
28 and 77, Laravel's framework 37 and 437, okhttp 35 and 145, bloc 51 and 167, http 20 and 58. Unresolved imports
left: 7 in 61 repositories (two stale Laravel imports, three `testdata` packages the scan excludes, two generated
Dart templates). The cycles reported (okhttp 1, framework 1, http 2) were not read by hand.

Limits: a Java or Kotlin import with no file of that name resolves to its package's files and is never reported as
broken, and an Elixir module may live in any file, so neither reports a broken import. Repositories with one
package and no sub-directory of sources stay one module (requests). Gradle (Groovy) and Swift manifests are not read.

Declared edges also come from `mix.exs` (the tuples of `deps`, and `path:` for a sibling application), from Kotlin DSL
builds (`project(":a:b")`, called by the last segment, so two projects with the same directory name are confused) and
from `Package.swift` (`Package(name:)` and `.package(path:)`; a Swift module is the package's name, which holds for
a package with one target). All three are read from the parse. A `build.gradle` in Groovy is not read.

## Dead code and public API from declarations

`detect_declared_dead_code` and `build_declared_public_api` read the semantic model: declarations with a
visibility (modifiers, or the name where the language says so: Go capitals, Python and Dart underscores) and every
name each file mentions. A private name is dead when its file never mentions it again, a package-visible one when no
file of its directory does, a repository-visible one when no file does; a public top-level name no other file
mentions is `exported-but-unused` at low confidence. Annotated declarations, `override` and `abstract` ones,
interface members, C# types and conventional entry-point names (`main`, `__init__`, `handle_*`) are left out.

Read by hand on flask, requests, http, eShop and okhttp: three real findings (a leftover `_make_timedelta`, `_path_is_ancestor`,
`_find`), and the false classes found and removed: Python underscore names imported across packages, Dart names
shared through `part` files, C# overrides, interface members and extension classes. eShop went from 132 to 4
findings, flask from 4 to 1.

Not solved: Laravel gives 509 findings because it calls methods by a name built at run time
(`'compile'.$name`); they are reported at low confidence with the number of such places, and a name search cannot do
better. Public members are not reported, the public surface is every public declaration (no re-export is followed),
and Swift, C++ and GDScript have no declarations.

## One analysis over every language

`analysis::analyze` runs every stage over a workspace and gives each module the reader of its own ecosystem: the entrypoints of a JavaScript package (`package.json`), the `pub` reachability of a Rust crate, and the declarations of any other language the semantic model reads, with their visibility from `data/visibility.toml`. A module has one surface. What no stage reads is returned in `not_covered` (C++ and GDScript have no module reader; Rust dead code is the compiler's lint) and a review reports it as not covered.

On the 61-repository corpus it takes 38 s for 364 modules and does not fail. Surfaces by language (modules, of them read): C# 18/18, Dart 51/51, Elixir 1/1, Go 9/9, Java 24/24, JavaScript 155/85, Kotlin 10/10, PHP 43/43, Python 13/13, Ruby 6/6, Rust 34/34. The 70 JavaScript modules not read hold no JavaScript file: a nested package whose files belong to its parent, or a directory of tests. Dead code is found in C#, Dart, Go, Java, JavaScript, Kotlin, PHP, Python, Ruby and TypeScript; 127 C++ and 69 GDScript files are not read.

Three defects came out of running it on every language: a Swift package was not analyzed at all (the extension was not in the list the stages read, and Swift had no declarations in its adapter), an import of a package at the repository root made an edge to a module that did not exist and stopped the cycle search, and a surface said it was read when the language had no visibility rule. The three are fixed and have tests (`tests/polyglot_analysis.rs`, `tests/ecosystems.rs`, `tests/declared_api.rs`).

The comparison with the TypeScript engine (`just corpus-analysis`) explains each difference by a rule: a module, an edge, a name or a cycle that belongs to an ecosystem the TypeScript engine does not read.

