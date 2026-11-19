// biome-ignore-all lint/suspicious/noTemplateCurlyInString: examples of code that contains ${} in a string
import type { RecommendationAction } from '@debuggatha/core';

/**
 * What a finding tells the person (or agent) who has to act on it: why the
 * pattern is a problem, what to change, and the same code before and after.
 * A rule with no entry falls back to the pack's own statement.
 */
export interface Guidance {
  why: string;
  action: RecommendationAction;
  fix: string;
  before: string;
  after: string;
}

export const GUIDANCE: Record<string, Guidance> = {
  'no-explicit-any': {
    why: '`any` switches off type checking for everything it touches, so a wrong value type-checks and fails at runtime.',
    action: 'replace',
    fix: 'Type the value, or use `unknown` and narrow it before use.',
    before: 'function parse(input: any) { return input.id; }',
    after:
      'function parse(input: unknown) {\n  if (typeof input === "object" && input !== null && "id" in input) return input.id;\n}',
  },
  'strict-null-checks': {
    why: 'A non-null assertion (`!`) tells the compiler a value cannot be null without checking, and crashes when it is.',
    action: 'replace',
    fix: 'Handle the null case, or use optional chaining with a fallback.',
    before: 'const name = user!.name;',
    after: 'const name = user?.name ?? "anonymous";',
  },
  'no-var': {
    why: '`var` is function-scoped and hoisted, so a variable can be read before it is assigned or leak out of its block.',
    action: 'replace',
    fix: 'Use `const`, or `let` if the value is reassigned.',
    before: 'var total = 0;',
    after: 'let total = 0;',
  },
  eqeqeq: {
    why: "`==` converts types before comparing (`0 == ''` is true), which hides bugs. Comparing to `null` with `== null` is the one accepted idiom and is not reported.",
    action: 'replace',
    fix: 'Compare with `===` or `!==`.',
    before: 'if (count == 0) {}',
    after: 'if (count === 0) {}',
  },
  'no-eval': {
    why: '`eval` runs a string as code, so anything that reaches it can run arbitrary code in your process.',
    action: 'replace',
    fix: 'Parse data with `JSON.parse`, and call functions directly instead of building code from strings.',
    before: 'const value = eval(expression);',
    after: 'const value = JSON.parse(expression);',
  },
  'no-implied-eval': {
    why: '`new Function(...)` and string timers compile a string into code, with the same risk as `eval`.',
    action: 'replace',
    fix: 'Pass a function, not a string.',
    before: 'setTimeout("refresh()", 1000);',
    after: 'setTimeout(() => refresh(), 1000);',
  },
  'prefer-template-literals': {
    why: 'Building strings with `+` gets hard to read and easy to break as it grows.',
    action: 'replace',
    fix: 'Use a template literal.',
    before: 'const greeting = "Hello, " + name + "!";',
    after: 'const greeting = `Hello, ${name}!`;',
  },
  'prefer-object-spread': {
    why: '`Object.assign({}, a)` is the long way to copy an object.',
    action: 'replace',
    fix: 'Use object spread.',
    before: 'const copy = Object.assign({}, options);',
    after: 'const copy = { ...options };',
  },
  'no-extend-native': {
    why: 'Changing a built-in prototype affects every library in the process, and can break them or be broken by them.',
    action: 'replace',
    fix: 'Write a standalone helper function instead of adding to the prototype.',
    before: 'Array.prototype.last = function () { return this[this.length - 1]; };',
    after: 'const last = <T>(items: T[]) => items[items.length - 1];',
  },
  'avoid-print': {
    why: '`print` output is unstructured and cannot be filtered or turned off in release builds.',
    action: 'replace',
    fix: 'Use `debugPrint`, or a logger.',
    before: "print('Loaded $count items');",
    after: "debugPrint('Loaded $count items');",
  },
  'no-hardcoded-secrets': {
    why: 'A secret in source code is in every clone, every fork, and the git history forever.',
    action: 'replace',
    fix: 'Read it from the environment or a secret manager, and rotate the exposed value.',
    before: 'const apiKey = "sk_live_...";',
    after: 'const apiKey = process.env.API_KEY;',
  },
  'no-dangerously-set-inner-html': {
    why: '`dangerouslySetInnerHTML` renders a string as HTML, so untrusted text becomes a script-injection hole.',
    action: 'replace',
    fix: 'Render the value as text, or sanitize it with a vetted library first.',
    before: '<div dangerouslySetInnerHTML={{ __html: comment }} />',
    after: '<div>{comment}</div>',
  },
  'use-v-html-carefully': {
    why: '`v-html` renders a string as HTML, so untrusted text becomes a script-injection hole.',
    action: 'replace',
    fix: 'Render the value as text with `{{ }}`, or sanitize it first.',
    before: '<div v-html="comment"></div>',
    after: '<div>{{ comment }}</div>',
  },
  'no-unwrap-expect': {
    why: '`unwrap()` and `expect()` panic on `None` or `Err`, taking the whole thread down.',
    action: 'replace',
    fix: 'Propagate the error with `?`, or handle the failure case.',
    before: 'let file = File::open(path).unwrap();',
    after: 'let file = File::open(path)?;',
  },
  'no-unsafe-blocks': {
    why: "`unsafe` turns off the compiler's memory-safety guarantees for that block.",
    action: 'investigate',
    fix: 'Check whether safe code can do the same; if not, document the invariant the block relies on.',
    before: 'unsafe { *ptr = value; }',
    after:
      '// SAFETY: `ptr` is valid and uniquely borrowed for this write.\nunsafe { *ptr = value; }',
  },
  'no-panic': {
    why: '`panic` aborts the current goroutine or thread; a library that panics takes its caller down with it.',
    action: 'replace',
    fix: 'Return an error and let the caller decide.',
    before: 'panic("config missing")',
    after: 'return fmt.Errorf("config missing")',
  },
  'no-focused-tests': {
    why: 'A focused test (`.only`, `fit`) makes the runner skip every other test, so CI stays green while the rest of the suite stops running.',
    action: 'remove',
    fix: 'Remove `.only` (or `f` prefix) before committing.',
    before: "it.only('adds', () => { expect(add(1, 2)).toBe(3); });",
    after: "it('adds', () => { expect(add(1, 2)).toBe(3); });",
  },
  'no-empty-tests': {
    why: 'An empty test always passes, so it counts as coverage while verifying nothing.',
    action: 'replace',
    fix: 'Write the assertions, or mark it `it.todo` so the gap stays visible.',
    before: "it('rejects bad input', () => {});",
    after: "it.todo('rejects bad input');",
  },
  'no-assertionless-tests': {
    why: 'A test with no assertion passes unless the code throws, so it cannot catch a wrong result.',
    action: 'replace',
    fix: 'Assert on the result. If a helper asserts for you, this is a false positive: suppress it with a reason.',
    before: "it('adds', () => { add(1, 2); });",
    after: "it('adds', () => { expect(add(1, 2)).toBe(3); });",
  },
  'async-route-errors': {
    why: 'In Express 4 a rejected promise in an async handler is not forwarded to the error middleware, so the request hangs.',
    action: 'replace',
    fix: 'Wrap the body in try/catch and call `next(err)`, or use an async wrapper. Express 5 handles this itself.',
    before: "app.get('/u', async (req, res) => { res.json(await load()); });",
    after:
      "app.get('/u', async (req, res, next) => {\n  try { res.json(await load()); } catch (err) { next(err); }\n});",
  },
  'no-async-foreach': {
    why: '`forEach` ignores the promises an `async` callback returns, so the loop ends before the work does and failures become unhandled rejections.',
    action: 'replace',
    fix: 'Use `for...of` with `await` for sequential work, or `await Promise.all(items.map(...))` for concurrent work.',
    before: 'items.forEach(async (item) => { await save(item); });',
    after: 'await Promise.all(items.map((item) => save(item)));',
  },
  'no-shell-interpolation': {
    why: '`exec` runs the string in a shell, so a value containing `;` or `$(...)` runs as a command.',
    action: 'replace',
    fix: 'Use `execFile` or `spawn` with an argument array so no shell reads the values.',
    before: 'exec(`git clone ${url}`);',
    after: "execFile('git', ['clone', url]);",
  },
  'no-open-cors': {
    why: "Allowing every origin lets any website call the API from a visitor's browser, with their cookies if credentials are on.",
    action: 'configure',
    fix: 'Pass an explicit allow-list as `origin`.',
    before: 'app.use(cors());',
    after: "app.use(cors({ origin: ['https://app.example.com'] }));",
  },
  'no-error-leak': {
    why: 'Stack traces and raw messages show paths, versions and queries to whoever triggers the error.',
    action: 'replace',
    fix: 'Log the error on the server and send a generic message.',
    before: 'res.status(500).json({ error: err.stack });',
    after: "logger.error(err);\nres.status(500).json({ error: 'Internal error' });",
  },
};

export function guidanceFor(ruleId: string): Guidance | undefined {
  return GUIDANCE[ruleId];
}
