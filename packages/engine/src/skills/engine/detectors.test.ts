// biome-ignore-all lint/suspicious/noTemplateCurlyInString: examples of code that contains ${} in a string
import { describe, expect, it } from 'bun:test';
import { type DetectorHit, detectorFor } from './detectors.js';
import { lex } from './lex.js';

function detect(
  ruleId: string,
  file: string,
  content: string,
  lineNumbers?: number[],
): DetectorHit[] {
  const detector = detectorFor(ruleId);
  if (!detector) throw new Error(`no detector for ${ruleId}`);
  const lexed = lex(content, file);
  return detector({
    file,
    content,
    lineNumbers,
    masked: lexed.masked,
    strings: lexed.strings,
  });
}

describe('RULE_DETECTORS', () => {
  it('detects `any` usage only in TS files', () => {
    const hits = detect('no-explicit-any', 'src/foo.ts', 'function f(x: any) {\n  return x;\n}');
    expect(hits).toHaveLength(1);
    expect(hits[0]?.lines).toEqual({ start: 1, end: 1 });
    expect(detect('no-explicit-any', 'src/foo.js', 'function f(x) { return x; }')).toHaveLength(0);
  });

  it('maps hits back to real line numbers via lineNumbers', () => {
    const hits = detect('no-var', 'src/foo.js', 'var a = 1;', [42]);
    expect(hits[0]?.lines).toEqual({ start: 42, end: 42 });
  });

  it('reports the columns a match spans', () => {
    const [hit] = detect('no-eval', 'a.js', '  const v = eval(input);');
    expect(hit?.columns).toEqual({ start: 13, end: 18 });
  });

  it('flags eval() and setTimeout with a string argument', () => {
    expect(detect('no-eval', 'a.js', 'eval(userInput)')).toHaveLength(1);
    expect(detect('no-eval', 'a.js', 'setTimeout("doSomething()", 100)')).toHaveLength(1);
  });

  it('has no detector for rules that require type/structural info', () => {
    expect(detectorFor('no-floating-promises')).toBeUndefined();
    expect(detectorFor('require-await')).toBeUndefined();
    expect(detectorFor('consistent-type-imports')).toBeUndefined();
  });

  it('gives each detector an honest confidence', () => {
    expect(detect('no-eval', 'a.js', 'eval(x)')[0]?.confidence).toBe('high');
    expect(detect('strict-null-checks', 'a.ts', 'const n = user!.name;')[0]?.confidence).toBe(
      'medium',
    );
    expect(detect('prefer-template-literals', 'a.js', 'const s = "a" + b;')[0]?.confidence).toBe(
      'low',
    );
  });
});

describe('code, not text: strings and comments are never matched', () => {
  it("ignores a rule's pattern inside a string", () => {
    expect(detect('no-eval', 'a.ts', 'const doc = "never call eval(x)";')).toHaveLength(0);
    expect(detect('eqeqeq', 'a.ts', 'const s = "if (a == b)";')).toHaveLength(0);
    expect(detect('no-var', 'a.js', "const s = 'var x = 1';")).toHaveLength(0);
  });

  it("ignores a rule's pattern inside a comment", () => {
    expect(detect('no-eval', 'a.ts', '// eval(x) is banned here')).toHaveLength(0);
    expect(detect('eqeqeq', 'a.ts', '/* a == b */ const ok = a === b;')).toHaveLength(0);
    expect(detect('no-var', 'a.js', '// var legacy = true;')).toHaveLength(0);
  });

  it('does not flag a method that merely shares the name', () => {
    expect(detect('no-eval', 'a.ts', 'await redis.eval(script, 0);')).toHaveLength(0);
    expect(detect('no-implied-eval', 'a.ts', 'vm.new Function(x)')).toHaveLength(0);
  });

  it('still flags real code on the same line as a string that mentions it', () => {
    expect(detect('no-eval', 'a.ts', 'log("careful"); eval(input);')).toHaveLength(1);
  });
});

describe('equality: == null is an idiom, not a bug', () => {
  it('accepts comparison to null in either order', () => {
    expect(detect('eqeqeq', 'a.ts', 'if (val == null) return null;')).toHaveLength(0);
    expect(detect('eqeqeq', 'a.ts', 'if (val != null) use(val);')).toHaveLength(0);
    expect(detect('eqeqeq', 'a.ts', 'if (null == val) return;')).toHaveLength(0);
  });

  it('still flags loose equality against anything else', () => {
    expect(detect('eqeqeq', 'a.ts', 'if (count == 0) {}')).toHaveLength(1);
    expect(detect('eqeqeq', 'a.ts', 'if (a != undefined) {}')).toHaveLength(1);
  });

  it('does not confuse strict equality, comparisons, or arrows with loose equality', () => {
    for (const code of ['a === b', 'a !== b', 'a <= b', 'a >= b', 'const f = (x) => x']) {
      expect(detect('eqeqeq', 'a.ts', code)).toHaveLength(0);
    }
  });
});

describe('hardcoded secrets', () => {
  it('flags a credential that names its own format, in any file, as critical', () => {
    const [hit] = detect(
      'no-hardcoded-secrets',
      'config.yaml',
      'api_key: "sk_live_1234567890abcdef"',
    );
    expect(hit?.severity).toBe('critical');
    expect(hit?.confidence).toBe('high');
  });

  it('reports the same credential in test code, but as a fixture: medium, not critical', () => {
    const [hit] = detect(
      'no-hardcoded-secrets',
      'src/pay.test.ts',
      'const key = "sk_live_Z9y8X7w6V5u4T3s2R1q0P";', // synthetic fixture
    );
    expect(hit?.severity).toBe('medium');
    expect(hit?.confidence).toBe('medium');
  });

  it('recognizes common token formats', () => {
    const samples = [
      'AKIAIOSFODNN7ABCDEFG',
      `ghp_${'a1B2'.repeat(9)}`,
      '-----BEGIN RSA PRIVATE KEY-----',
      `xoxb-${'1234567890'.repeat(2)}`,
    ];
    for (const sample of samples) {
      expect(detect('no-hardcoded-secrets', 'x.env', `KEY=${sample}`)).toHaveLength(1);
    }
  });

  it('never echoes a secret back in the excerpt', () => {
    const [known] = detect('no-hardcoded-secrets', 'a.ts', 'const k = "sk_live_1234567890abcdef";');
    expect(known?.excerpt).not.toContain('1234567890abcdef');
    const [named] = detect('no-hardcoded-secrets', 'a.ts', 'const password = "hunter22222";');
    expect(named?.excerpt).not.toContain('hunter22222');
    expect(named?.excerpt).toContain('[redacted]');
  });

  it('does not read a name as a secret: constants whose value is a label', () => {
    const kotlin = [
      'private const val KEY_ACCESS_TOKEN = "offline_access_token"',
      'private const val KEY_REFRESH_TOKEN = "offline_refresh_token"',
      'const val PASSWORD_FIELD = "password"',
      'val tokenType = "Bearer"',
    ].join('\n');
    expect(detect('no-hardcoded-secrets', 'core/Store.kt', kotlin)).toHaveLength(0);
  });

  it('does not flag fake credentials in test code, or placeholders anywhere', () => {
    expect(
      detect(
        'no-hardcoded-secrets',
        'server/src/test/kotlin/AuthTest.kt',
        'password = "hunter22222"',
      ),
    ).toHaveLength(0);
    for (const line of [
      'password = "test_password"',
      'const apiKey = "your_api_key_here"',
      'secret = "changeme"',
      'token = "<token>"',
      'password = "${DB_PASSWORD}"',
      'authUrl = "https://example.com/token"',
    ]) {
      expect(detect('no-hardcoded-secrets', 'src/config.ts', line)).toHaveLength(0);
    }
  });

  it('still flags a real-looking password or high-entropy key assigned to a secret name', () => {
    const password = detect('no-hardcoded-secrets', 'src/db.ts', 'const password = "hunter22222";');
    expect(password).toHaveLength(1);
    expect(password[0]?.severity).toBe('high');
    expect(password[0]?.confidence).toBe('low');

    const key = detect(
      'no-hardcoded-secrets',
      'src/api.ts',
      '{ "apiKey": "8f3k2n9dLx0Qz7vB1mYpTqW4" }',
    );
    expect(key).toHaveLength(1);
    expect(key[0]?.confidence).toBe('medium');
  });
});

describe('other precision guards', () => {
  it('flags `var` in a for loop, not a comment', () => {
    expect(detect('no-var', 'a.js', 'for (var i = 0; i < 3; i++) {}')).toHaveLength(1);
  });

  it('flags Object.assign only when it copies into a fresh object', () => {
    expect(
      detect('prefer-object-spread', 'a.js', 'const c = Object.assign({}, opts);'),
    ).toHaveLength(1);
    expect(detect('prefer-object-spread', 'a.js', 'Object.assign(target, opts);')).toHaveLength(0);
  });

  it('lets Rust tests unwrap and panic', () => {
    expect(detect('no-unwrap-expect', 'tests/parse.rs', 'let x = maybe.unwrap();')).toHaveLength(0);
    expect(detect('no-panic', 'tests/parse.rs', 'panic!("bad")')).toHaveLength(0);
  });
});

describe('detector expansion', () => {
  it('detects dangerouslySetInnerHTML only in tsx/jsx files', () => {
    expect(
      detect(
        'no-dangerously-set-inner-html',
        'a.tsx',
        '<div dangerouslySetInnerHTML={{__html: x}} />',
      ),
    ).toHaveLength(1);
    expect(detect('no-dangerously-set-inner-html', 'a.ts', 'dangerouslySetInnerHTML')).toHaveLength(
      0,
    );
  });

  it('detects v-html only in .vue files', () => {
    expect(detect('use-v-html-carefully', 'a.vue', '<div v-html="raw"></div>')).toHaveLength(1);
    expect(detect('use-v-html-carefully', 'a.ts', 'v-html="raw"')).toHaveLength(0);
  });

  it('detects .unwrap()/.expect() only in .rs files', () => {
    expect(detect('no-unwrap-expect', 'a.rs', 'let x = maybe.unwrap();')).toHaveLength(1);
    expect(detect('no-unwrap-expect', 'a.rs', 'let x = maybe.expect("boom");')).toHaveLength(1);
    expect(detect('no-unwrap-expect', 'a.ts', 'maybe.unwrap()')).toHaveLength(0);
  });

  it('detects unsafe blocks/functions only in .rs files', () => {
    expect(detect('no-unsafe-blocks', 'a.rs', 'unsafe { ptr.read() }')).toHaveLength(1);
    expect(detect('no-unsafe-blocks', 'a.rs', 'unsafe fn danger() {}')).toHaveLength(1);
  });

  it("detects Rust's panic! macro and Go's panic() call, distinguished by extension", () => {
    expect(detect('no-panic', 'a.rs', 'panic!("bad")')).toHaveLength(1);
    expect(detect('no-panic', 'a.go', 'panic("bad")')).toHaveLength(1);
    expect(detect('no-panic', 'a.go', 'panic!("bad")')).toHaveLength(0);
    expect(detect('no-panic', 'a.ts', 'panic("bad")')).toHaveLength(0);
  });
});

describe('test-suite quality detectors', () => {
  it('flags focused tests, and only calls', () => {
    expect(detect('no-focused-tests', 'a.test.ts', "it.only('x', () => {});")).toHaveLength(1);
    expect(detect('no-focused-tests', 'a.test.ts', "describe.only('x', () => {});")).toHaveLength(
      1,
    );
    expect(detect('no-focused-tests', 'a.spec.js', "fit('x', () => {});")).toHaveLength(1);
    expect(detect('no-focused-tests', 'a.test.ts', "it('x', () => {});")).toHaveLength(0);
    expect(detect('no-focused-tests', 'a.test.ts', 'model.fit(data)')).toHaveLength(0);
    expect(detect('no-focused-tests', 'a.test.ts', "// it.only('x', f)")).toHaveLength(0);
    expect(detect('no-focused-tests', 'a.test.ts', 'const s = "it.only(";')).toHaveLength(0);
  });

  it('flags empty tests, including comment-only bodies, but not todo or skip', () => {
    expect(detect('no-empty-tests', 'a.test.ts', "it('x', () => {});")).toHaveLength(1);
    expect(
      detect('no-empty-tests', 'a.test.ts', "test('x', async function () {\n  // later\n});"),
    ).toHaveLength(1);
    expect(detect('no-empty-tests', 'a.test.ts', "it.todo('x');")).toHaveLength(0);
    expect(detect('no-empty-tests', 'a.test.ts', "it.skip('x', () => {});")).toHaveLength(0);
    expect(detect('no-empty-tests', 'a.test.ts', "it('x', () => expect(1).toBe(1));")).toHaveLength(
      0,
    );
    expect(detect('no-empty-tests', 'src/a.ts', "it('x', () => {});")).toHaveLength(0);
  });

  it('flags tests with no assertion, at low confidence', () => {
    const hits = detect('no-assertionless-tests', 'a.test.ts', "it('x', () => {\n  run();\n});");
    expect(hits).toHaveLength(1);
    expect(hits[0]?.confidence).toBe('low');
    expect(hits[0]?.lines).toEqual({ start: 1, end: 1 });
    expect(
      detect(
        'no-assertionless-tests',
        'a.test.ts',
        "it('x', () => {\n  expect(run()).toBe(1);\n});",
      ),
    ).toHaveLength(0);
    expect(detect('no-assertionless-tests', 'a.test.ts', "it('x', () => {});")).toHaveLength(0);
    expect(
      detect(
        'no-assertionless-tests',
        'a.test.ts',
        "test('x', async () => { await assertValid(); });",
      ),
    ).toHaveLength(0);
  });

  it('flags unguarded async Express handlers', () => {
    const bad = "app.get('/u', async (req, res) => {\n  res.json(await load());\n});";
    expect(detect('async-route-errors', 'a.ts', bad)).toHaveLength(1);
    expect(
      detect(
        'async-route-errors',
        'a.ts',
        "app.get('/u', async (req, res) => { try { await f(); } catch (e) { next(e); } });",
      ),
    ).toHaveLength(0);
    expect(
      detect(
        'async-route-errors',
        'a.ts',
        "app.get('/u', wrap(async (req, res) => { await f(); }));",
      ),
    ).toHaveLength(0);
    expect(detect('async-route-errors', 'a.ts', "app.get('port')")).toHaveLength(0);
    expect(detect('async-route-errors', 'a.test.ts', bad)).toHaveLength(0);
  });
});

describe('Phase 1 safety-net detectors', () => {
  it('flags async forEach callbacks, also across lines', () => {
    expect(
      detect('no-async-foreach', 'a.ts', 'items.forEach(async (i) => { await f(i); });'),
    ).toHaveLength(1);
    expect(
      detect('no-async-foreach', 'a.ts', 'items.forEach(\n  async (i) => {},\n);'),
    ).toHaveLength(1);
    expect(detect('no-async-foreach', 'a.ts', 'items.forEach((i) => f(i));')).toHaveLength(0);
    expect(detect('no-async-foreach', 'a.ts', 'items.forEach(handleAsync);')).toHaveLength(0);
  });

  it('flags exec with an interpolated or concatenated command', () => {
    expect(detect('no-shell-interpolation', 'a.ts', 'exec(`git clone ${url}`);')).toHaveLength(1);
    expect(detect('no-shell-interpolation', 'a.ts', "execSync('ls ' + dir);")).toHaveLength(1);
    expect(detect('no-shell-interpolation', 'a.ts', 'cp.exec(`ls ${dir}`, cb);')).toHaveLength(1);
    expect(detect('no-shell-interpolation', 'a.ts', "exec('ls -la');")).toHaveLength(0);
    expect(detect('no-shell-interpolation', 'a.ts', 'execFile("git", [`${url}`]);')).toHaveLength(
      0,
    );
    expect(detect('no-shell-interpolation', 'a.ts', 'const m = re.exec(`${s}`);')).toHaveLength(0);
  });

  it('flags open CORS but not an allow-list', () => {
    expect(detect('no-open-cors', 'a.ts', 'app.use(cors());')).toHaveLength(1);
    expect(detect('no-open-cors', 'a.ts', "app.use(cors({ origin: '*' }));")).toHaveLength(1);
    expect(
      detect('no-open-cors', 'a.ts', "app.use(cors({ origin: ['https://a.com'] }));"),
    ).toHaveLength(0);
    expect(detect('no-open-cors', 'a.ts', "app.use(cors({ origin: ' ' }));")).toHaveLength(0);
  });

  it('flags error details sent to the client, stack as high confidence', () => {
    const stack = detect('no-error-leak', 'a.ts', 'res.status(500).json({ error: err.stack });');
    expect(stack).toHaveLength(1);
    expect(stack[0]?.confidence).toBe('high');
    expect(detect('no-error-leak', 'a.ts', 'res.send(error.message);')[0]?.confidence).toBe(
      'medium',
    );
    expect(
      detect(
        'no-error-leak',
        'a.ts',
        "logger.error(err.stack); res.status(500).json({ error: 'x' });",
      ),
    ).toHaveLength(0);
  });
});
