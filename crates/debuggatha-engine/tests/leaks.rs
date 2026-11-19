//! `leak-mock-restore` is specified by the cases below. Each is a set of files
//! (the review, which may hold a test runner's configuration), the file under test, and the
//! expected hits `(line, column)`. A positive is an override nothing puts back; a negative is the
//! look-alike that must stay quiet. Mutations of the detector must fail at least one of them.
#![allow(clippy::expect_used, clippy::unwrap_used)]
// a failed expect is a failed test
// The tests are tables of cases: splitting a table only hides what it covers.
#![allow(clippy::too_many_lines)]

use std::path::Path;

use debuggatha_engine::detect::modules::ModuleIndex;
use debuggatha_engine::detect::{DetectorInput, SourceUnit, leaks};
use debuggatha_engine::syntax::parse;

type Expected<'a> = &'a [(u32, u32)];
type Files<'a> = &'a [(&'a str, &'a str)];

fn run(target: &str, files: Files<'_>) -> Vec<(u32, u32)> {
    let units: Vec<SourceUnit> = files
        .iter()
        .map(|(file, content)| SourceUnit {
            file: (*file).to_owned(),
            content: (*content).to_owned(),
            changed_lines: None,
        })
        .collect();
    let index = ModuleIndex::build(&units);
    let source = files
        .iter()
        .find(|(f, _)| *f == target)
        .expect("the target is one of the files")
        .1;
    let syntax = parse(Path::new(target), source).expect("a supported language");
    let input = DetectorInput::new(target, source, Some(&syntax)).with_modules(Some(&index));
    leaks::detect(&input)
        .into_iter()
        .map(|(rule, h)| {
            assert_eq!(rule, "leak-mock-restore");
            assert_eq!(
                serde_json::to_value(h.confidence).unwrap().as_str(),
                Some("medium")
            );
            (h.line, h.columns.start)
        })
        .collect()
}

fn check(cases: &[(&str, &str, Files<'_>, Expected<'_>)]) {
    let mut bad = Vec::new();
    for (name, target, files, want) in cases {
        let got = run(target, files);
        if got != *want {
            bad.push(format!("{name}: got {got:?}, want {want:?}"));
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

/// A test file with one `it` whose body is `body`, on line 2.
macro_rules! it_body {
    ($body:expr) => {
        concat!("it('x', () => {\n", $body, "\n});\n")
    };
}

#[test]
fn spies_nothing_puts_back() {
    check(&[
        (
            "vi.spyOn",
            "a.test.ts",
            &[("a.test.ts", it_body!("  vi.spyOn(console, 'log');"))],
            &[(2, 3)],
        ),
        (
            "jest.spyOn",
            "a.test.ts",
            &[("a.test.ts", it_body!("  jest.spyOn(console, 'log');"))],
            &[(2, 3)],
        ),
        (
            "sinon spy, stub and mock",
            "a.test.js",
            &[(
                "a.test.js",
                "it('x', () => {\n  sinon.spy(o, 'a');\n  sinon.stub(o, 'b');\n  sinon.mock(o);\n});\n",
            )],
            &[(2, 3), (3, 3), (4, 3)],
        ),
        (
            "kept in a variable and never put back",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!("  const s = vi.spyOn(console, 'log');\n  s.mockReturnValue(1);"),
            )],
            &[(2, 13)],
        ),
        (
            "chained with an implementation",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!("  const s = vi.spyOn(o, 'a').mockImplementation(() => 1);"),
            )],
            &[(2, 13)],
        ),
        (
            "in a beforeEach with nothing after it",
            "a.test.ts",
            &[(
                "a.test.ts",
                "beforeEach(() => {\n  vi.spyOn(o, 'a');\n});\n",
            )],
            &[(2, 3)],
        ),
        (
            "at the top of the file it is the file's setup",
            "a.test.ts",
            &[(
                "a.test.ts",
                "vi.spyOn(o, 'a');\nvi.stubGlobal('x', 1);\nvi.stubEnv('K', 'v');\n",
            )],
            &[],
        ),
        (
            "in a describe callback it is the suite's setup",
            "a.test.ts",
            &[(
                "a.test.ts",
                "describe('s', () => {\n  vi.spyOn(o, 'a');\n  describe('t', function () {\n    vi.stubGlobal('x', 1);\n  });\n});\n",
            )],
            &[],
        ),
        (
            "describe.each and describe.skip are describe too",
            "a.test.ts",
            &[(
                "a.test.ts",
                "describe.each([1])('s', () => {\n  vi.spyOn(o, 'a');\n});\ndescribe.skip('s', () => {\n  vi.spyOn(o, 'b');\n});\n",
            )],
            &[],
        ),
        (
            "in a beforeAll it is the suite's setup",
            "a.test.ts",
            &[("a.test.ts", "beforeAll(() => {\n  vi.spyOn(o, 'a');\n});\n")],
            &[],
        ),
        (
            "test.beforeAll and mocha's before are the suite's setup",
            "a.test.ts",
            &[(
                "a.test.ts",
                "test.beforeAll(() => {\n  vi.spyOn(o, 'a');\n});\nbefore(() => {\n  sinon.stub(o, 'b');\n});\n",
            )],
            &[],
        ),
        (
            "vi.hoisted is setup that runs before the imports",
            "a.test.ts",
            &[(
                "a.test.ts",
                "vi.hoisted(() => {\n  vi.stubGlobal('X', 1);\n  vi.stubEnv('K', 'v');\n});\n",
            )],
            &[],
        ),
        (
            "an object that is new in every test",
            "a.test.ts",
            &[(
                "a.test.ts",
                "let dom;\nbeforeEach(() => {\n  dom = new JSDOM();\n});\nit('x', () => {\n  vi.spyOn(dom.window.document, 'a');\n});\n",
            )],
            &[],
        ),
        (
            "an object given a value at its declaration is shared",
            "a.test.ts",
            &[(
                "a.test.ts",
                "let dom = new JSDOM();\nbeforeEach(() => {\n  dom = new JSDOM();\n});\nit('x', () => {\n  vi.spyOn(dom, 'a');\n});\n",
            )],
            &[(6, 3)],
        ),
        (
            "an object also assigned elsewhere is not fresh",
            "a.test.ts",
            &[(
                "a.test.ts",
                "let dom;\nbeforeEach(() => {\n  dom = new JSDOM();\n});\nbeforeAll(() => {\n  dom = shared;\n});\nit('x', () => {\n  vi.spyOn(dom, 'a');\n});\n",
            )],
            &[(9, 3)],
        ),
        (
            "an object made once in a beforeAll is shared",
            "a.test.ts",
            &[(
                "a.test.ts",
                "let dom;\nbeforeAll(() => {\n  dom = new JSDOM();\n});\nit('x', () => {\n  vi.spyOn(dom, 'a');\n});\n",
            )],
            &[(6, 3)],
        ),
        (
            "an object a function returns may be shared",
            "a.test.ts",
            &[(
                "a.test.ts",
                "let dom;\nbeforeEach(() => {\n  dom = makeDom();\n});\nit('x', () => {\n  vi.spyOn(dom, 'a');\n});\n",
            )],
            &[(6, 3)],
        ),
        (
            "a helper function is not the suite's setup",
            "a.test.ts",
            &[("a.test.ts", "function mockIt() {\n  vi.spyOn(o, 'a');\n}\n")],
            &[(2, 3)],
        ),
        (
            "a test inside a describe is still a test",
            "a.test.ts",
            &[(
                "a.test.ts",
                "describe('s', () => {\n  it('x', () => {\n    vi.spyOn(o, 'a');\n  });\n});\n",
            )],
            &[(3, 5)],
        ),
        (
            "a spy on an object the test makes",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!("  const app = new Hono();\n  vi.spyOn(app, 'fetch');"),
            )],
            &[],
        ),
        (
            "a spy on a member of an object the test makes",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!(
                    "  const dom = makeDom();\n  const s = vi.spyOn(dom.window.document, 'createElement');"
                ),
            )],
            &[],
        ),
        (
            "a spy on a parameter of the test or of a callback in it",
            "a.test.ts",
            &[(
                "a.test.ts",
                "it('x', (ctx) => {\n  vi.spyOn(ctx.logger, 'a');\n  run((stream) => {\n    vi.spyOn(stream, 'close');\n  });\n});\n",
            )],
            &[],
        ),
        (
            "a spy in a callback on an object the test makes",
            "a.test.ts",
            &[(
                "a.test.ts",
                "it('x', () => {\n  const app = new Hono();\n  run(() => {\n    vi.spyOn(app, 'fetch');\n  });\n});\n",
            )],
            &[],
        ),
        (
            "a spy through a computed member and parentheses",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!(
                    "  const app = make();\n  vi.spyOn(app['inner'], 'a');\n  vi.spyOn((app).inner, 'b');"
                ),
            )],
            &[],
        ),
        (
            "a local name given to stubGlobal does not make it local",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!("  const name = 'fetch';\n  vi.stubGlobal(name, f);"),
            )],
            &[(3, 3)],
        ),
        (
            "an alias of the global is the global",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!("  const c = console;\n  vi.spyOn(c, 'log');"),
            )],
            &[(3, 3)],
        ),
        (
            "an object declared outside the test",
            "a.test.ts",
            &[(
                "a.test.ts",
                "const shared = makeShared();\nit('x', () => {\n  vi.spyOn(shared, 'a');\n});\n",
            )],
            &[(3, 3)],
        ),
        (
            "an object imported by the file",
            "a.test.ts",
            &[(
                "a.test.ts",
                "import { svc } from './svc';\nit('x', () => {\n  vi.spyOn(svc, 'a');\n});\n",
            )],
            &[(3, 3)],
        ),
        (
            "a global object member",
            "a.test.ts",
            &[("a.test.ts", it_body!("  vi.spyOn(global.Intl, 'Locale');"))],
            &[(2, 3)],
        ),
        (
            "a stubbed global is not made local by a local object",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!("  const f = vi.fn();\n  vi.stubGlobal('fetch', f);"),
            )],
            &[(3, 3)],
        ),
        (
            "another spy's restore does not restore this one",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!(
                    "  const a = vi.spyOn(o, 'a');\n  const b = vi.spyOn(o, 'b');\n  b.mockRestore();"
                ),
            )],
            &[(2, 13)],
        ),
        (
            "a restore of something the tree cannot tie to the spy",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!("  vi.spyOn(o, 'a');\n  getSpy().mockRestore();"),
            )],
            &[(2, 3)],
        ),
        (
            "a restore with an argument is not one",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!("  const s = vi.spyOn(o, 'a');\n  s.mockRestore(1);"),
            )],
            &[(2, 13)],
        ),
        (
            "two on a line are one hit",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!("  vi.spyOn(o, 'a'); vi.spyOn(o, 'b');"),
            )],
            &[(2, 3)],
        ),
        (
            "not a test file",
            "a.ts",
            &[("a.ts", it_body!("  vi.spyOn(o, 'a');"))],
            &[],
        ),
        (
            "a mock function is not a spy",
            "a.test.ts",
            &[("a.test.ts", it_body!("  const f = vi.fn();\n  jest.fn();"))],
            &[],
        ),
    ]);
}

#[test]
fn spies_something_puts_back() {
    check(&[
        (
            "mockRestore on the variable",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!("  const s = vi.spyOn(o, 'a');\n  s.mockRestore();"),
            )],
            &[],
        ),
        (
            "restore (sinon) on the variable",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!("  const s = sinon.spy(o, 'a');\n  s.restore();"),
            )],
            &[],
        ),
        (
            "clear and reset count, as the rule says",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!(
                    "  const a = vi.spyOn(o, 'a');\n  const b = vi.spyOn(o, 'b');\n  a.mockClear();\n  b.mockReset();"
                ),
            )],
            &[],
        ),
        (
            "assigned in a hook, restored in another",
            "a.test.ts",
            &[(
                "a.test.ts",
                "let spy;\nbeforeEach(() => {\n  spy = vi.spyOn(o, 'a');\n});\nafterEach(() => {\n  spy.mockRestore();\n});\n",
            )],
            &[],
        ),
        (
            "kept in a property",
            "a.test.ts",
            &[(
                "a.test.ts",
                "beforeEach(function () {\n  this.spy = vi.spyOn(o, 'a');\n});\nafterEach(function () {\n  this.spy.mockRestore();\n});\n",
            )],
            &[],
        ),
        (
            "restoreAllMocks of vi and jest",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterEach(() => { vi.restoreAllMocks(); });\nit('x', () => {\n  vi.spyOn(o, 'a');\n});\n",
            )],
            &[],
        ),
        (
            "jest.restoreAllMocks",
            "a.test.ts",
            &[(
                "a.test.ts",
                "it('x', () => {\n  jest.spyOn(o, 'a');\n  jest.restoreAllMocks();\n});\n",
            )],
            &[],
        ),
        (
            "clearAllMocks and resetAllMocks",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterEach(() => { jest.clearAllMocks(); });\nit('x', () => { jest.spyOn(o, 'a'); });\n",
            )],
            &[],
        ),
        (
            "sinon.restore in the test itself",
            "a.test.js",
            &[(
                "a.test.js",
                "it('x', () => {\n  sinon.stub(o, 'a');\n  sinon.restore();\n});\n",
            )],
            &[],
        ),
        (
            "resetAllMocks",
            "a.test.ts",
            &[(
                "a.test.ts",
                "it('x', () => {\n  vi.spyOn(o, 'a');\n  vi.resetAllMocks();\n});\n",
            )],
            &[],
        ),
        (
            "a property restored in the test itself",
            "a.test.ts",
            &[(
                "a.test.ts",
                "it('x', function () {\n  this.spy = vi.spyOn(o, 'a');\n  this.spy.mockRestore();\n});\n",
            )],
            &[],
        ),
        (
            "sinon.restore",
            "a.test.js",
            &[(
                "a.test.js",
                "afterEach(() => { sinon.restore(); });\nit('x', () => { sinon.stub(o, 'a'); });\n",
            )],
            &[],
        ),
        (
            "a restore in a hook, with the spies in an array",
            "a.test.ts",
            &[(
                "a.test.ts",
                "const spies = [];\nafterEach(() => {\n  spies.forEach((s) => s.mockRestore());\n});\nit('x', () => {\n  spies.push(vi.spyOn(o, 'a'));\n});\n",
            )],
            &[],
        ),
        (
            "await using",
            "a.test.ts",
            &[(
                "a.test.ts",
                "it('x', async () => {\n  await using s = vi.spyOn(o, 'a');\n});\n",
            )],
            &[],
        ),
        (
            "using",
            "a.test.ts",
            &[("a.test.ts", it_body!("  using s = vi.spyOn(o, 'a');"))],
            &[],
        ),
        (
            "restoreMocks in the root configuration",
            "a.test.ts",
            &[
                ("a.test.ts", it_body!("  vi.spyOn(o, 'a');")),
                (
                    "vitest.config.ts",
                    "export default { test: { restoreMocks: true } };\n",
                ),
            ],
            &[],
        ),
        (
            "restoreMocks set to false",
            "a.test.ts",
            &[
                ("a.test.ts", it_body!("  vi.spyOn(o, 'a');")),
                (
                    "vitest.config.ts",
                    "export default { test: { restoreMocks: false } };\n",
                ),
            ],
            &[(2, 3)],
        ),
        (
            "clearMocks, resetMocks and mockReset in a jest config",
            "a.test.ts",
            &[
                ("a.test.ts", it_body!("  jest.spyOn(o, 'a');")),
                ("jest.config.js", "module.exports = { clearMocks: true };\n"),
            ],
            &[],
        ),
        (
            "clearMocks in package.json",
            "a.test.ts",
            &[
                ("a.test.ts", it_body!("  jest.spyOn(o, 'a');")),
                ("package.json", "{ \"jest\": { \"clearMocks\": true } }\n"),
            ],
            &[],
        ),
        (
            "package.json without jest settings",
            "a.test.ts",
            &[
                ("a.test.ts", it_body!("  jest.spyOn(o, 'a');")),
                ("package.json", "{ \"name\": \"x\" }\n"),
            ],
            &[(2, 3)],
        ),
        (
            "jest settings in package.json",
            "a.test.ts",
            &[
                ("a.test.ts", it_body!("  jest.spyOn(o, 'a');")),
                ("package.json", "{ \"jest\": { \"restoreMocks\": true } }\n"),
            ],
            &[],
        ),
        (
            "a configuration in a package covers that package only",
            "packages/b/a.test.ts",
            &[
                ("packages/b/a.test.ts", it_body!("  vi.spyOn(o, 'a');")),
                (
                    "packages/a/vitest.config.ts",
                    "export default { test: { restoreMocks: true } };\n",
                ),
            ],
            &[(2, 3)],
        ),
        (
            "a configuration in the package of the file",
            "packages/a/src/a.test.ts",
            &[
                ("packages/a/src/a.test.ts", it_body!("  vi.spyOn(o, 'a');")),
                (
                    "packages/a/vitest.config.ts",
                    "export default { test: { restoreMocks: true } };\n",
                ),
            ],
            &[],
        ),
        (
            "a configuration in vite.config",
            "a.test.ts",
            &[
                ("a.test.ts", it_body!("  vi.spyOn(o, 'a');")),
                (
                    "vite.config.ts",
                    "export default defineConfig({ test: { restoreMocks: true } });\n",
                ),
            ],
            &[],
        ),
        (
            "a file that is not a configuration",
            "a.test.ts",
            &[
                ("a.test.ts", it_body!("  vi.spyOn(o, 'a');")),
                ("settings.ts", "export default { restoreMocks: true };\n"),
            ],
            &[(2, 3)],
        ),
    ]);
}

#[test]
fn stubbed_globals_and_environment() {
    check(&[
        (
            "stubGlobal",
            "a.test.ts",
            &[("a.test.ts", it_body!("  vi.stubGlobal('fetch', f);"))],
            &[(2, 3)],
        ),
        (
            "stubGlobal with unstubAllGlobals",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterEach(() => { vi.unstubAllGlobals(); });\nit('x', () => { vi.stubGlobal('fetch', f); });\n",
            )],
            &[],
        ),
        (
            "restoreAllMocks does not unstub a global",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterEach(() => { vi.restoreAllMocks(); });\nit('x', () => { vi.stubGlobal('fetch', f); });\n",
            )],
            &[(2, 17)],
        ),
        (
            "unstubGlobals in the configuration",
            "a.test.ts",
            &[
                ("a.test.ts", it_body!("  vi.stubGlobal('fetch', f);")),
                (
                    "vitest.config.ts",
                    "export default { test: { unstubGlobals: true } };\n",
                ),
            ],
            &[],
        ),
        (
            "restoreMocks does not unstub a global",
            "a.test.ts",
            &[
                ("a.test.ts", it_body!("  vi.stubGlobal('fetch', f);")),
                (
                    "vitest.config.ts",
                    "export default { test: { restoreMocks: true } };\n",
                ),
            ],
            &[(2, 3)],
        ),
        (
            "stubbed again in an afterEach is the reset",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterEach(() => {\n  vi.stubGlobal('X', 0);\n});\nit('x', () => {\n  vi.stubGlobal('X', 1);\n  vi.stubGlobal('Y', 1);\n});\n",
            )],
            &[(6, 3)],
        ),
        (
            "a stub in an afterEach is not an override",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterEach(() => {\n  vi.stubGlobal('X', 0);\n  vi.stubEnv('K', 'v');\n});\n",
            )],
            &[],
        ),
        (
            "an environment variable stubbed again in an afterEach",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterEach(() => {\n  vi.stubEnv('K', old);\n});\nit('x', () => {\n  vi.stubEnv('K', 'v');\n  vi.stubEnv('J', 'v');\n});\n",
            )],
            &[(6, 3)],
        ),
        (
            "a stub of a computed name in an afterEach covers any name",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterEach(() => {\n  for (const n of names) vi.stubGlobal(n, undefined);\n});\nit('x', () => {\n  vi.stubGlobal('Y', 1);\n});\n",
            )],
            &[],
        ),
        (
            "a global stubbed again is not an environment variable put back",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterEach(() => {\n  vi.stubGlobal('K', 0);\n});\nit('x', () => {\n  vi.stubEnv('K', 'v');\n});\n",
            )],
            &[(5, 3)],
        ),
        (
            "stubEnv",
            "a.test.ts",
            &[("a.test.ts", it_body!("  vi.stubEnv('K', 'v');"))],
            &[(2, 3)],
        ),
        (
            "stubEnv with unstubAllEnvs",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterEach(() => { vi.unstubAllEnvs(); });\nit('x', () => { vi.stubEnv('K', 'v'); });\n",
            )],
            &[],
        ),
        (
            "unstubEnvs in the configuration",
            "a.test.ts",
            &[
                ("a.test.ts", it_body!("  vi.stubEnv('K', 'v');")),
                (
                    "vitest.config.ts",
                    "export default { test: { unstubEnvs: true } };\n",
                ),
            ],
            &[],
        ),
        (
            "unstubAllGlobals does not unstub the environment",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterEach(() => { vi.unstubAllGlobals(); });\nit('x', () => { vi.stubEnv('K', 'v'); });\n",
            )],
            &[(2, 17)],
        ),
    ]);
}

#[test]
fn process_env_written_in_a_test() {
    check(&[
        (
            "assigned in a test",
            "a.test.ts",
            &[("a.test.ts", it_body!("  process.env.K = 'v';"))],
            &[(2, 3)],
        ),
        (
            "a computed key",
            "a.test.ts",
            &[("a.test.ts", it_body!("  process.env[k] = 'v';"))],
            &[(2, 3)],
        ),
        (
            "the whole object replaced in a beforeEach",
            "a.test.ts",
            &[(
                "a.test.ts",
                "beforeEach(() => {\n  process.env = { ...old, K: 'v' };\n});\n",
            )],
            &[(2, 3)],
        ),
        (
            "at the top of the file it is the file's setup",
            "a.test.ts",
            &[("a.test.ts", "process.env.NODE_ENV = 'test';\n")],
            &[],
        ),
        (
            "put back by the same key in an afterEach",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterEach(() => { process.env.K = old; });\nit('x', () => {\n  process.env.K = 'v';\n});\n",
            )],
            &[],
        ),
        (
            "put back by a delete in an afterEach",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterEach(() => { delete process.env.K; });\nit('x', () => {\n  process.env.K = 'v';\n});\n",
            )],
            &[],
        ),
        (
            "put back by a delete with a computed key in an afterEach",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterEach(() => {\n  for (const k of changed) delete process.env[k];\n});\nfunction set(k, v) {\n  process.env[k] = v;\n}\n",
            )],
            &[],
        ),
        (
            "set in a describe callback is the suite's setup",
            "a.test.ts",
            &[(
                "a.test.ts",
                "describe('s', () => {\n  process.env.K = 'v';\n});\n",
            )],
            &[],
        ),
        (
            "put back by replacing the object in an afterAll",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterAll(() => { process.env = old; });\nit('x', () => {\n  process.env.K = 'v';\n});\n",
            )],
            &[],
        ),
        (
            "another key in the afterEach does not put it back",
            "a.test.ts",
            &[(
                "a.test.ts",
                "afterEach(() => { process.env.OTHER = old; });\nit('x', () => {\n  process.env.K = 'v';\n});\n",
            )],
            &[(3, 3)],
        ),
        (
            "a restore in a beforeEach is not an after hook",
            "a.test.ts",
            &[(
                "a.test.ts",
                "beforeEach(() => { process.env.K = old; });\nit('x', () => {\n  process.env.K = 'v';\n});\n",
            )],
            &[(1, 20), (3, 3)],
        ),
        (
            "set and put back in the same test",
            "a.test.ts",
            &[(
                "a.test.ts",
                "it('x', () => {\n  const prev = process.env.K;\n  process.env.K = 'v';\n  try {\n    run();\n  } finally {\n    process.env.K = prev;\n  }\n});\n",
            )],
            &[],
        ),
        (
            "set and deleted in the same test",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!("  process.env.K = 'v';\n  delete process.env.K;"),
            )],
            &[],
        ),
        (
            "the same key set in two tests is two overrides",
            "a.test.ts",
            &[(
                "a.test.ts",
                "it('a', () => {\n  process.env.K = 'v';\n});\nit('b', () => {\n  process.env.K = 'v';\n});\n",
            )],
            &[(2, 3), (5, 3)],
        ),
        (
            "the object put back whole in the same test",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!(
                    "  const old = process.env;\n  process.env.K = 'v';\n  process.env = old;"
                ),
            )],
            &[],
        ),
        (
            "a computed key is not put back by writing another key",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!("  process.env[k] = 'v';\n  process.env.K = 'w';"),
            )],
            &[(2, 3), (3, 3)],
        ),
        (
            "two different keys in one test",
            "a.test.ts",
            &[(
                "a.test.ts",
                it_body!("  process.env.A = 'v';\n  process.env.B = 'v';"),
            )],
            &[(2, 3), (3, 3)],
        ),
        (
            "a write in an after hook is not an override",
            "a.test.ts",
            &[("a.test.ts", "afterEach(() => { process.env.K = old; });\n")],
            &[],
        ),
        (
            "reading is not writing",
            "a.test.ts",
            &[("a.test.ts", it_body!("  const v = process.env.K;"))],
            &[],
        ),
        (
            "comparison is not writing",
            "a.test.ts",
            &[("a.test.ts", it_body!("  if (process.env.K === 'v') run();"))],
            &[],
        ),
    ]);
}
