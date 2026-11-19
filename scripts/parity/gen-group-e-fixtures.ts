// Writes the differential fixtures for the Rust group E detector (`no-hardcoded-secrets`) by
// running the TypeScript detector over small sources. Run: bun scripts/parity/gen-group-e-fixtures.ts
//
// Every token here is built from pieces and is not a credential. A case is "same" when Rust must
// give exactly what TypeScript gave (line, columns, confidence, severity and excerpt), or "differs"
// when the tree is right and the lexer wrong; a "differs" case says why and states the Rust hits as
// [rule, line, startColumn].
import { writeFileSync } from 'node:fs';
import {
  type DetectorInput,
  RULE_DETECTORS,
} from '../../packages/engine/src/skills/engine/detectors.js';
import { lex } from '../../packages/engine/src/skills/engine/lex.js';

const RULE = 'no-hardcoded-secrets';

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

const aws = `AK${'IA'}QWERTYUIOPASDFGH`;
const gh = `gh${'p_'}${'aB3dE6gH9jK2mN5pQ8sT1vW4yZ7bC0eF3hJ6'}`;
const jwt = `ey${'J'}hbGciOiJIUzI1NiJ9.ey${'J'}zdWIiOiIxMjM0NTY3ODkwIn0.dBjftJeZ4CVPmB92K27uhbUJU1p1r`;
const pem = `-----BEGIN ${'RSA '}PRIVATE KEY-----`;
const pemBody = `MIIEvQIBADANBgkqhkiG9w0BAQEFAASC${'BKcwggSjAgEAAoIBAQC7VJTUt9Us8cKj'}`;
const T = 'a.test.ts';

const cases: Case[] = [
  // --- a credential that names its own format
  same('aws-key', 'a.ts', `const k = "${aws}";\n`),
  same('github-token', 'a.js', `const t = '${gh}';\n`),
  same('jwt', 'a.js', `const t = "${jwt}";\n`),
  same('pem-with-body', 'a.txt', `${pem}\n${pemBody}\n`),
  differs(
    'pem-header-alone',
    'a.kt',
    `append("${pem}\\n")\nappend(base64Encoded)\n`,
    'a header written by code that generates a key is not a key: it counts when a base64 body follows',
    [],
  ),
  same('token-in-comment', 'a.ts', `// ${aws}\n`),
  same('token-unparsed-file', 'a.yaml', `key: ${aws}\n`),
  same('token-example-skipped', 'a.ts', 'const k = "AKIAIOSFODNN7EXAMPLE";\n'),
  same('token-two-on-a-line', 'a.ts', `const a = "${aws}", b = "${gh}";\n`),
  same('token-unicode-before', 'a.ts', `const s = "😀"; const k = "${aws}";\n`),
  same('token-crlf', 'a.ts', `const a = 1;\r\nconst k = "${aws}";\r\n`),
  same('token-long-line-excerpt', 'a.ts', `const k = "${aws}"; ${'x'.repeat(300)}\n`),
  same('token-in-test-file', T, `const k = "${aws}";\n`),
  same('token-in-test-dir', 'tests/a.ts', `const k = "${aws}";\n`),
  same('token-synthetic-in-test', T, 'const k = "AKIA1234567890ABCDEF";\n'),
  same('token-fixture-line-in-test', T, `const fixture = "${aws}";\n`),
  same('token-python', 'a.py', `KEY = "${aws}"\n`),

  // --- a literal assigned to a name that says it is a secret
  same('password-weak', 'a.js', 'const password = "hunter2hunter2";\n'),
  same('apikey-strong', 'a.js', 'const apiKey = "a8Kd93jfL0zQ7xPw";\n'),
  same('property', 'a.ts', 'const o = { token: "abcdEFGH1234ijkl" };\n'),
  same('quoted-key', 'a.ts', 'const o = { "password": "abcdefgh1234" };\n'),
  same('arrow-name', 'a.ts', 'const f = password => "abcdefgh1234";\n'),
  same('too-short', 'a.js', 'const password = "abc12";\n'),
  same('placeholder-changeme', 'a.js', 'const password = "changeme123";\n'),
  same('placeholder-your', 'a.js', 'const apiKey = "your-api-key-here";\n'),
  same('placeholder-interpolation', 'a.js', 'const password = "${PASSWORD}ab";\n'),
  same('placeholder-xs', 'a.js', 'const password = "xxxxxxxx";\n'),
  same('words', 'a.js', 'const password = "Invalid password";\n'),
  same('snake-words', 'a.js', 'const token = "access_token";\n'),
  same('url', 'a.js', 'const apiToken = "https://example.org/x";\n'),
  same('path', 'a.js', 'const secretFile = "/etc/secret.key";\n'),
  same('value-is-name', 'a.js', 'const authKey = "authkey";\n'),
  same('name-not-secret', 'a.js', 'const name = "qwertyuiop12345";\n'),
  same('strong-needed-for-non-password', 'a.js', 'const authKey = "abcabcabcabc";\n'),
  same('strong-entropy-exactly-three', 'a.js', 'const authKey = "a1b2c3d4a1b2c3d4";\n'),
  same('single-word', 'a.js', 'const password = "abcdefghij";\n'),
  same('long-single-word', 'a.js', 'const password = "abcdefghijklmnopqrstuvwxyzab";\n'),
  same('value-is-name-with-digit', 'a.js', 'const password1 = "password1";\n'),
  same('weak-value-non-password-name', 'a.js', 'const apiKey = "abc123def456";\n'),
  same('rust-error-region', 'a.rs', 'fn f( { let password = "hunter2hunter2"; }\nfn g() {}\n'),
  same('strong-non-password', 'a.js', 'const authKey = "a8Kd93jfL0zQ7xPw";\n'),
  same('test-file-skips-assignments', T, 'const password = "hunter2hunter2";\n'),
  same('two-on-a-line', 'a.js', 'const password = "hunter2hunter2", token = "abcdEFGH1234ijkl";\n'),
  same('after-known-token-on-line', 'a.js', `const password = "${aws}";\n`),
  same('in-comment', 'a.js', '// password = "hunter2hunter2"\n'),
  same('escaped-quote', 'a.js', 'const password = "ab\\"cdefgh1234";\n'),
  same('astral-value', 'a.js', 'const password = "😀😀😀😀😀😀";\n'),
  same('unicode-before', 'a.js', 'const s = "😀"; const password = "hunter2hunter2";\n'),
  same('excerpt-redacts-value', 'a.ts', 'const password = "hunter2hunter2"; // keep\n'),
  same('python', 'a.py', 'password = "hunter2hunter2"\n'),
  same('python-dict', 'a.py', 'cfg = {"password": "hunter2hunter2"}\n'),
  same('python-triple', 'a.py', 'password = """hunter2hunter2"""\n'),
  same('java', 'A.java', 'class A { String password = "hunter2hunter2"; }\n'),
  same('csharp', 'A.cs', 'class A { string password = "hunter2hunter2"; }\n'),
  same('rust', 'a.rs', 'fn f() { let password = "hunter2hunter2"; }\n'),
  same(
    'go-short-declaration-not-matched',
    'a.go',
    'package m\nfunc f() { password := "hunter2hunter2" }\n',
  ),
  same('go-var', 'a.go', 'package m\nvar password = "hunter2hunter2"\n'),
  same('php', 'a.php', '<?php\n$password = "hunter2hunter2";\n'),
  same('ruby', 'a.rb', 'password = "hunter2hunter2"\n'),
  same('kotlin', 'a.kt', 'val password = "hunter2hunter2"\n'),
  same('dart', 'a.dart', "final password = 'hunter2hunter2';\n"),
  same('swift', 'a.swift', 'let password = "hunter2hunter2"\n'),
  differs(
    'fatal-parse-has-no-literals',
    'a.ts',
    'const o = { password => "abcdefgh1234" };\n',
    'a file the parser could not read at all has no string literals to judge (a known format is still found)',
    [],
  ),
  differs(
    'multiline-value-is-redacted',
    'a.js',
    'const password = `abcdefgh\n1234`;\n',
    'the old excerpt kept the start of a value that runs over lines, which put part of a secret in the ledger',
    [['no-hardcoded-secrets', 1, 18]],
  ),
  // --- values that are not credentials (the first measurement on the corpus, docs/DETECTOR_QUALITY.md)
  differs(
    'ui-text-accented',
    'a.js',
    'const confirmPassword = "Confirmar Contraseña";\n',
    'the word check knew only A-Z, so interface text with an accent looked like a secret',
    [],
  ),
  differs(
    'ui-sentence-with-comma',
    'a.js',
    'const passwordHint = "If omitted, the application key is used";\n',
    'a sentence of five words or more is interface text, commas included',
    [],
  ),
  same('pipe-separated-secret', 'a.js', 'const apiKey = "8kd93|jf9l0|zq7xp|w2rm5v";\n'),
  same('short-phrase-with-comma-stays', 'a.js', 'const secret = "shhhh, very secret";\n'),
  differs(
    'regex-shaped-value',
    'a.js',
    'const TOKEN = "([a-zA-Z0-9-!#$%&\'*+.^_{|}~]+)";\n',
    'a character class with a quantifier is a regular expression',
    [],
  ),
  differs(
    'token-prefix',
    'a.js',
    'const AUTH_TOKEN_PREFIX = "base64url.bearer.phx.";\n',
    'a value ending in a dot is a prefix to be completed, not a token',
    [],
  ),
  differs(
    'validation-rule-list',
    'a.php',
    "<?php\n$rules = ['password' => 'required|string|current_password'];\n",
    'a list of validation rules is not a password',
    [],
  ),
  differs(
    'password-hash',
    'a.js',
    'const password = "$argon2id$v=19$m=65536,t=3,p=4$wagCPXjifgvUFBzq4hqe3w$CYaIb8sB";\n',
    'a password hash is not a credential',
    [],
  ),
  differs(
    'ui-question',
    'a.js',
    'const forgotPassword = "Forgot your password?";\n',
    'a question mark ends a label; it is not part of a secret',
    [],
  ),
  differs(
    'ui-question-spanish',
    'a.js',
    'const forgotPassword = "¿Olvidaste tu contraseña?";\n',
    'the opening question mark is punctuation',
    [],
  ),
  differs(
    'ui-label-with-parentheses',
    'a.js',
    'const passwordLabel = "Contraseña (opcional)";\n',
    'a label with a parenthesis is words',
    [],
  ),
  differs(
    'cli-flag',
    'a.php',
    "<?php\n$args = ['password' => '--password=' . $connection['password']];\n",
    'a command-line flag is not a credential',
    [],
  ),
  differs(
    'kotlin-interpolation',
    'a.kt',
    'val usernameAndPassword = "$username:$password"\n',
    'a Kotlin string built with interpolation is computed, not hard-coded',
    [],
  ),
  differs(
    'dart-interpolation',
    'a.dart',
    "String get storageToken => '$storagePrefix$id';\n",
    'a Dart string built with interpolation is computed, not hard-coded',
    [],
  ),
  same('dart-dollar-not-interpolation', 'a.dart', "final password = 'ab9Xk$%2fQz7Lm!pR';\n"),
  same('python-fstring-prefix-not-judged', 'a.py', 'password = f"hunter2hunter2"\n'),
  same('rust-raw-string-not-judged', 'a.rs', 'fn f() { let password = r"hunter2hunter2"; }\n'),
  same(
    'dart-raw-character-table',
    'a.dart',
    'const _tokenChars = r"!#$%&\'*+-.0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ^_`"\n',
  ),
  differs(
    'vue-script',
    'a.vue',
    '<script>\nconst password = "hunter2hunter2";\n</script>\n',
    'the old lexer knew no component syntax and found no strings; the script is code',
    [['no-hardcoded-secrets', 2, 18]],
  ),
  differs(
    'mts-extension',
    'a.mts',
    'const password = "hunter2hunter2";\n',
    'the old lexer did not know .mts',
    [['no-hardcoded-secrets', 1, 18]],
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
  const ts = (RULE_DETECTORS[RULE]?.(input(c)) ?? []).map((h) => ({
    rule: RULE,
    line: h.index + 1,
    start: h.columns.start,
    end: h.columns.end,
    confidence: h.confidence,
    severity: h.severity,
    excerpt: h.excerpt,
  }));
  return { ...c, ts };
});

writeFileSync(
  'crates/debuggatha-engine/tests/fixtures/group-e-conformance.json',
  `${JSON.stringify({ rules: [RULE], cases: out }, null, 2)}\n`,
);
console.log(`${out.length} cases (${out.filter((c) => c.rust).length} differ)`);
