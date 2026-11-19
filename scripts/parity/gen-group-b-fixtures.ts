// Writes the differential fixtures for the Rust group B detectors (calls and tests of the
// JavaScript/TypeScript family) by running the TypeScript detectors over small sources.
// Run: bun scripts/parity/gen-group-b-fixtures.ts
//
// A case is "same" when the Rust detector must give exactly what the TypeScript one gave (rule,
// line, columns, confidence), or "differs" when the tree is right and the regex is wrong; a
// "differs" case says why and states the Rust hits as [rule, line, startColumn].
import { writeFileSync } from 'node:fs';
import {
  type DetectorInput,
  RULE_DETECTORS,
} from '../../packages/engine/src/skills/engine/detectors.js';
import { lex } from '../../packages/engine/src/skills/engine/lex.js';

const RULES = [
  'no-async-foreach',
  'no-shell-interpolation',
  'no-open-cors',
  'no-error-leak',
  'async-route-errors',
  'no-focused-tests',
  'no-empty-tests',
  'no-assertionless-tests',
];

type Case = {
  name: string;
  file: string;
  source: string;
  why?: string;
  rust?: [string, number, number][];
};
const same = (name: string, file: string, source: string): Case => ({ name, file, source });
const differs = (
  name: string,
  file: string,
  source: string,
  why: string,
  rust: [string, number, number][],
): Case => ({ name, file, source, why, rust });

const T = 'a.test.ts';

const cases: Case[] = [
  // --- no-async-foreach
  same('foreach-async-arrow', 'a.ts', 'items.forEach(async (x) => { await f(x); });\n'),
  same('foreach-async-function', 'a.js', 'items.forEach(async function (x) {});\n'),
  same('foreach-async-bare-param', 'a.js', 'items.forEach(async x => {});\n'),
  same('foreach-sync', 'a.js', 'items.forEach((x) => f(x));\n'),
  same('foreach-multiline', 'a.js', 'items\n  .forEach(\n    async (x) => {},\n  );\n'),
  same('foreach-optional-chain', 'a.js', 'items?.forEach(async x => {});\n'),
  same('foreach-space-before-paren', 'a.js', 'items.forEach (async x => {});\n'),
  same(
    'foreach-in-string-and-comment',
    'a.js',
    '// a.forEach(async x => {})\nconst s = "a.forEach(async x => 1)";\n',
  ),
  same('foreach-test-files-are-scanned', T, 'items.forEach(async x => {});\n'),
  same('foreach-vue-not-scanned', 'a.vue', '<script>\nitems.forEach(async x => {});\n</script>\n'),
  same('foreach-second-argument-async', 'a.js', 'items.forEach(f, async () => {});\n'),
  differs(
    'foreach-callback-named-async',
    'a.js',
    'items.forEach(async);\n',
    'async here is a name, not a function',
    [],
  ),

  // --- no-shell-interpolation
  same('exec-template', 'a.ts', 'exec(`ls ${dir}`);\n'),
  same('exec-sync-concat', 'a.ts', 'execSync("rm -rf " + dir);\n'),
  same('exec-cp-prefix', 'a.js', 'cp.exec(`ls ${d}`, cb);\n'),
  same('exec-child-process-prefix', 'a.js', 'child_process.execSync(`ls ${d}`);\n'),
  same('exec-childProcess-prefix', 'a.js', 'childProcess.exec("a " + b);\n'),
  same('exec-literal-template', 'a.js', 'exec(`ls -la`);\n'),
  same('exec-plain-string', 'a.js', 'exec("ls -la");\n'),
  same('exec-regex-method', 'a.js', 're.exec(`a${b}`);\n'),
  same('exec-other-object', 'a.js', 'db.exec(`a${b}`);\n'),
  same('exec-test-file-skipped', T, 'exec(`ls ${d}`);\n'),
  same('exec-multiline', 'a.js', 'exec(\n  `ls ${x}`,\n);\n'),
  same('exec-with-options', 'a.js', 'exec(`ls ${d}`, { cwd });\n'),
  same('exec-only-template', 'a.js', 'exec(`${cmd}`);\n'),
  same('exec-in-string-and-comment', 'a.js', '// exec(`a${b}`)\nconst s = "exec(`a${b}`)";\n'),
  same('exec-template-plus', 'a.js', 'exec(`ls` + dir);\n'),
  same('exec-crlf', 'a.js', 'exec(`ls ${d}`);\r\nexec("a" + b);\r\n'),
  differs(
    'exec-name-first-concat',
    'a.js',
    'exec(cmd + " -v");\n',
    'a command built from a value is read by the shell whichever side the text is on',
    [['no-shell-interpolation', 1, 1]],
  ),
  differs(
    'exec-constant-concat',
    'a.js',
    'exec("ls " + "-la");\n',
    'two constants hold no value for a shell to read',
    [],
  ),
  same('exec-call-inside-interpolation', 'a.js', 'exec(`ls ${a(b, c)}`);\n'),

  // --- no-open-cors
  same('cors-bare', 'a.js', 'app.use(cors());\n'),
  same('cors-spaces', 'a.js', 'app.use(cors(  ));\n'),
  same('cors-comment-inside', 'a.js', 'app.use(cors(/* all */));\n'),
  same('cors-with-origin', 'a.js', "app.use(cors({ origin: 'https://a.com' }));\n"),
  same('cors-origin-star', 'a.js', "app.use(cors({ origin: '*' }));\n"),
  same('cors-origin-star-double', 'a.js', 'const o = { origin: "*" };\n'),
  same('cors-origin-star-backtick', 'a.js', 'const o = { origin: `*` };\n'),
  same('cors-origin-star-spaced', 'a.ts', "const o = { origin   :   '*' };\n"),
  same('cors-member-call', 'a.js', 'x.cors();\n'),
  same('cors-test-file-skipped', T, 'app.use(cors());\n'),
  same('cors-origin-star-test-file-skipped', T, 'const o = { origin: "*" };\n'),
  same('cors-quoted-key', 'a.js', 'const o = { "origin": "*" };\n'),
  same('cors-in-comment-and-string', 'a.js', "// cors()\nconst s = 'cors()';\n"),
  same('cors-origin-other', 'a.js', "const o = { origin: '*.example.com' };\n"),
  same('cors-origin-empty', 'a.js', "const o = { origin: '' };\n"),
  differs(
    'cors-both-same-line',
    'a.js',
    "app.use(cors(), { origin: '*' });\n",
    'the old rule reported both on one line; a rule is reported once per line, as every other rule is, so the ledger identity of the first stays what it was',
    [['no-open-cors', 1, 9]],
  ),

  // --- no-error-leak
  same('leak-message', 'a.js', 'res.json({ error: err.message });\n'),
  same('leak-stack-status', 'a.js', 'res.status(500).json({ stack: err.stack });\n'),
  same('leak-send-e-stack', 'a.js', 'res.send(e.stack);\n'),
  same('leak-response-error', 'a.js', 'response.json(error.message);\n'),
  same('leak-ex', 'a.js', 'res.send(ex.message);\n'),
  same('leak-none', 'a.js', 'res.json({ ok: true });\n'),
  same('leak-first-wins-message', 'a.js', 'res.json({ m: err.message, s: err.stack });\n'),
  same('leak-first-wins-stack', 'a.js', 'res.json({ s: err.stack, m: err.message });\n'),
  same('leak-generic-message', 'a.js', 'res.status(500).send("Server error");\n'),
  same('leak-test-file-skipped', T, 'res.json(err.message);\n'),
  same('leak-multiline', 'a.js', 'res.status(500).json({\n  error: err.message,\n});\n'),
  same('leak-this-res', 'a.js', 'this.res.json(err.stack);\n'),
  same('leak-console', 'a.js', 'console.log(err.message);\n'),
  same('leak-messages-plural', 'a.js', 'res.json(err.messages);\n'),
  same('leak-nested-member-err', 'a.js', 'res.json(foo.err.message);\n'),
  same('leak-status-spaced', 'a.js', 'res . status ( 500 ) . json ( err.stack );\n'),
  same('leak-in-string', 'a.js', 'res.json("err.stack");\n'),
  differs(
    'leak-status-with-call',
    'a.js',
    'res.status(code()).json({ e: err.message });\n',
    'the old pattern could not read a call inside status()',
    [['no-error-leak', 1, 1]],
  ),
  differs(
    'leak-optional-chain',
    'a.js',
    'res.json({ e: err?.message });\n',
    'err?.message reads the same text',
    [['no-error-leak', 1, 1]],
  ),

  // --- async-route-errors
  same('route-async-arrow', 'a.js', "app.get('/', async (req, res) => { await x(); });\n"),
  same(
    'route-async-function',
    'a.js',
    "router.post('/x', async function (req, res) { await y(); });\n",
  ),
  same(
    'route-with-try',
    'a.js',
    "app.get('/', async (req, res) => { try { await x(); } catch (e) {} });\n",
  ),
  same(
    'route-with-catch-call',
    'a.js',
    "app.get('/', async (req, res) => { x().catch(next); });\n",
  ),
  same('route-sync', 'a.js', "app.get('/', (req, res) => { res.send(1); });\n"),
  same('route-suffix-router', 'a.ts', "userRouter.get('/', async (req, res) => { await x(); });\n"),
  same('route-suffix-app', 'a.ts', "adminApp.put('/', async (req, res) => { await x(); });\n"),
  same('route-capital-router-alone', 'a.js', "Router.get('/', async () => { await x(); });\n"),
  same('route-other-object', 'a.js', "server.get('/', async () => { await x(); });\n"),
  same('route-all-method', 'a.js', "app.all('*', async (req, res) => { await x(); });\n"),
  same(
    'route-middleware-before',
    'a.js',
    "app.get('/x', auth, async (req, res) => { await x(); });\n",
  ),
  same(
    'route-multiline',
    'a.js',
    "app.get(\n  '/',\n  async (req, res) => {\n    await x();\n  },\n);\n",
  ),
  same('route-test-file-skipped', T, "app.get('/', async () => { await x(); });\n"),
  same('route-wrapped-async', 'a.js', "app.get('/', wrap(async (req, res) => { await x(); }));\n"),
  same('route-this-app', 'a.js', "this.app.get('/', async () => { await x(); });\n"),
  same('route-use-not-a-route', 'a.js', 'app.use(async (req, res, next) => { await x(); });\n'),
  same('route-parenthesized', 'a.js', "app.get('/', (async () => { await x(); }));\n"),
  same('route-in-string', 'a.js', 'const s = "app.get(\'/\', async () => {})";\n'),
  same(
    'route-try-in-nested-function',
    'a.js',
    "app.get('/', async () => { const f = () => { try { g(); } catch {} }; await f(); });\n",
  ),
  same(
    'route-ts',
    'a.ts',
    "app.post('/', async (req: Request, res: Response) => { await x(); });\n",
  ),

  // --- no-focused-tests
  same('only-it', 'a.js', "it.only('x', () => {});\n"),
  same('only-describe', 'a.js', "describe.only('x', () => {});\n"),
  same('only-test', 'a.ts', "test.only('x', () => {});\n"),
  same('only-context-suite', 'a.js', "context.only('x', () => {});\nsuite.only('y', () => {});\n"),
  same('only-fit-fdescribe', 'a.js', "fit('x', () => {});\nfdescribe('y', () => {});\n"),
  same('only-spaced', 'a.js', "it.only ('x', () => {});\n"),
  same('only-in-string-and-comment', 'a.js', "// it.only('x')\nconst s = \"it.only('x')\";\n"),
  same('only-member-of-object', 'a.js', "foo.it.only('x', () => {});\n"),
  same('only-skip-is-fine', 'a.js', "it.skip('x', () => {});\nit('y', () => {});\n"),
  same('only-fit-not-called', 'a.js', 'const fit = 1;\nconst g = fit + 1;\n'),
  same('only-in-test-file', T, "it.only('x', () => { expect(1); });\n"),
  same('only-in-non-test-file', 'a.ts', "it.only('x', () => {});\n"),
  same('only-two-on-one-line', 'a.js', "it.only('a', () => {}); it.only('b', () => {});\n"),
  differs(
    'only-each',
    'a.js',
    "describe.only.each([1])('x', () => {});\n",
    'a focused table is focused',
    [['no-focused-tests', 1, 1]],
  ),
  differs(
    'only-playwright-describe',
    'a.js',
    "test.describe.only('x', () => {});\n",
    'a focused Playwright group is focused',
    [['no-focused-tests', 1, 1]],
  ),

  // --- no-empty-tests (test files only)
  same('empty-arrow', T, "it('x', () => {});\n"),
  same('empty-async', T, "test('x', async () => {});\n"),
  same('empty-function', T, "it('x', function () {});\n"),
  same('empty-only', T, "it.only('x', () => {});\n"),
  same('empty-comment-only', T, "it('x', () => { /* todo */ });\n"),
  same('empty-line-comment', T, "it('x', () => {\n  // todo\n});\n"),
  same('empty-expression-body', T, "it('x', () => expect(1));\n"),
  same('empty-no-callback', T, "it('x');\n"),
  same('empty-done-param', T, "it('x', done => {});\n"),
  same('empty-with-timeout', T, "it('x', () => {}, 1000);\n"),
  same('empty-name-not-string', T, 'it(name, () => {});\n'),
  same('empty-template-name', T, 'it(`x ${y}`, () => {});\n'),
  same('empty-each-not-matched', T, "it.each([1])('x', () => {});\n"),
  same('empty-wrapped-callback', T, "it('x', wrap(() => {}));\n"),
  same('empty-nested', T, "describe('x', () => {\n  it('y', () => {});\n});\n"),
  same('empty-non-test-file', 'a.ts', "it('x', () => {});\n"),
  same('empty-spec-file', 'a.spec.js', "it('x', () => {});\n"),
  same('empty-test-dir', '__tests__/a.js', "it('x', () => {});\n"),
  same('empty-use-strict', T, "it('x', () => { 'use strict'; });\n"),
  same('empty-concurrent', T, "it.concurrent('x', async () => {});\n"),
  same('empty-two-lines', T, "it('a', () => {});\nit('b', () => {});\n"),
  same('empty-in-string', T, 'const s = "it(\'x\', () => {})";\n'),

  // --- no-assertionless-tests
  same('noassert-plain', T, "it('x', () => { doThing(); });\n"),
  same('noassert-expect', T, "it('x', () => { expect(1).toBe(1); });\n"),
  same('noassert-assert', T, "it('x', () => { assert.ok(1); });\n"),
  same('noassert-should', T, "it('x', () => { x.should.equal(1); });\n"),
  same('noassert-verify', T, "it('x', () => { verify(x); });\n"),
  same('noassert-fail', T, "it('x', () => { fail('no'); });\n"),
  same('noassert-done-call', T, "it('x', (done) => { f(done); });\n"),
  same('noassert-rejects', T, "it('x', async () => { await p.rejects; });\n"),
  same('noassert-throw', T, "it('x', () => { f(() => g()).toThrow(); });\n"),
  same('noassert-snapshot', T, "it('x', () => { f().toMatchSnapshot(); });\n"),
  same('noassert-helper-named-expect', T, "it('x', () => { expectEvent(x); });\n"),
  same('noassert-comment-mentions', T, "it('x', () => {\n  // expect nothing\n  f();\n});\n"),
  same('noassert-string-mentions', T, "it('x', () => { f('expect'); });\n"),
  same('noassert-done-param-unused', T, "it('x', (done) => { f(); });\n"),
  same('noassert-uppercase', T, "it('x', () => { EXPECT(x); });\n"),
  same('noassert-nested-callback', T, "it('x', () => { run(() => { expect(1); }); });\n"),
  same('noassert-non-test-file', 'a.ts', "it('x', () => { doThing(); });\n"),
  same('noassert-use-strict', T, "it('x', () => { 'use strict'; });\n"),
  same('noassert-multiline', T, "it('x', async () => {\n  await a();\n  await b();\n});\n"),
  same('noassert-expression-body', T, "it('x', () => run());\n"),
  same('noassert-object-key', T, "it('x', () => { f({ expected: 1 }); });\n"),
  same('noassert-await-expect', T, "it('x', async () => { await expect(p).resolves.toBe(1); });\n"),

  // --- layout
  same('crlf-lines', T, "it.only('a', () => {});\r\nit('b', () => { f(); });\r\n"),
  same('unicode-before', 'a.js', 'const s = "😀"; items.forEach(async x => {});\n'),
  same('recoverable-error-elsewhere', 'a.js', 'const q;\nitems.forEach(async x => {});\n'),
  differs(
    'fatal-error-file-is-not-analyzed',
    'a.js',
    "const q = ;\nit.only('x', () => {});\n",
    'nothing is concluded about a file the parser could not read',
    [],
  ),
];

const input = (c: Case): DetectorInput => {
  const lexed = lex(c.source, c.file);
  return {
    file: c.file,
    content: c.source,
    lineNumbers: undefined,
    masked: lexed.masked,
    strings: lexed.strings,
  };
};

const out = cases.map((c) => {
  const ts = RULES.flatMap((rule) =>
    (RULE_DETECTORS[rule]?.(input(c)) ?? []).map((h) => ({
      rule,
      line: h.index + 1,
      start: h.columns.start,
      end: h.columns.end,
      confidence: h.confidence,
    })),
  );
  return { ...c, ts };
});

writeFileSync(
  'crates/debuggatha-engine/tests/fixtures/group-b-conformance.json',
  `${JSON.stringify({ rules: RULES, cases: out }, null, 2)}\n`,
);
console.log(`${out.length} cases (${out.filter((c) => c.rust).length} differ)`);
