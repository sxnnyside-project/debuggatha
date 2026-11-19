// Writes the differential fixtures for the Rust group D detector (`use-v-html-carefully`, the
// template of a Vue component) by running the TypeScript regex detector over small sources.
// Run: bun scripts/parity/gen-group-d-fixtures.ts
//
// A case is "same" when Rust must give exactly what TypeScript gave, or "differs" when the tree is
// right and the regex is wrong; a "differs" case says why and states the Rust hits as
// [rule, line, startColumn].
import { writeFileSync } from 'node:fs';
import {
  type DetectorInput,
  RULE_DETECTORS,
} from '../../packages/engine/src/skills/engine/detectors.js';
import { lex } from '../../packages/engine/src/skills/engine/lex.js';

const RULES = ['use-v-html-carefully'];

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

const cases: Case[] = [
  same('basic', 'a.vue', '<template>\n  <div v-html="html"></div>\n</template>\n'),
  same('single-quotes', 'a.vue', "<template><p v-html='h'></p></template>\n"),
  same('unquoted', 'a.vue', '<template><p v-html=h></p></template>\n'),
  same('spaces-around-equals', 'a.vue', '<template><p v-html  =  "h"></p></template>\n'),
  same(
    'other-attributes-first',
    'a.vue',
    '<template><p class="a" id="b" v-html="h"></p></template>\n',
  ),
  same(
    'two-elements',
    'a.vue',
    '<template>\n<p v-html="a"></p>\n<p v-html="b"></p>\n</template>\n',
  ),
  same(
    'two-on-one-line-one-hit',
    'a.vue',
    '<template><p v-html="a"></p><p v-html="b"></p></template>\n',
  ),
  same('self-closing', 'a.vue', '<template><Comp v-html="x" /></template>\n'),
  same('nested', 'a.vue', '<template><div><ul><li v-html="x"></li></ul></div></template>\n'),
  same('unicode-before', 'a.vue', '<template><p title="😀" v-html="x"></p></template>\n'),
  same('crlf', 'a.vue', '<template>\r\n  <p v-html="x"></p>\r\n</template>\r\n'),
  same('no-value', 'a.vue', '<template><p v-html></p></template>\n'),
  same('v-text-is-fine', 'a.vue', '<template><p v-text="x"></p></template>\n'),
  same(
    'with-script',
    'a.vue',
    '<script setup>\nconst h = "<b>x</b>";\n</script>\n<template><p v-html="h"></p></template>\n',
  ),
  same('html-file-not-scanned', 'a.html', '<p v-html="x"></p>\n'),
  same('svelte-not-scanned', 'a.svelte', '<p v-html="x"></p>\n'),
  same('ts-file-not-scanned', 'a.ts', 'const s = `v-html="x"`;\n'),
  differs(
    'in-script-string',
    'a.vue',
    '<script>\nconst s = \'v-html="x"\';\n</script>\n',
    'the old lexer did not mask strings in a component, so text in a script was read as an attribute',
    [],
  ),
  differs(
    'in-script-comment',
    'a.vue',
    '<script>\n// v-html="x"\n</script>\n',
    'the old lexer did not mask comments in a component, so text in a script was read as an attribute',
    [],
  ),
  differs(
    'colon-prefix',
    'a.vue',
    '<template><p :v-html="x"></p></template>\n',
    ':v-html is a binding to a prop of that name, not the directive',
    [],
  ),
  differs(
    'data-prefix',
    'a.vue',
    '<template><p data-v-html="x"></p></template>\n',
    'data-v-html is another attribute',
    [],
  ),
  differs(
    'html-comment',
    'a.vue',
    '<template>\n  <!-- <p v-html="x"></p> -->\n</template>\n',
    'text inside an HTML comment is not code',
    [],
  ),
  differs(
    'equals-on-next-line',
    'a.vue',
    '<template><p v-html\n  ="x"></p></template>\n',
    'the old pattern read one line at a time',
    [['use-v-html-carefully', 1, 14]],
  ),
  differs(
    'in-style-block',
    'a.vue',
    '<style>\n.a::after { content: "v-html=1"; }\n</style>\n',
    'a style block holds no template',
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
  'crates/debuggatha-engine/tests/fixtures/group-d-conformance.json',
  `${JSON.stringify({ rules: RULES, cases: out }, null, 2)}\n`,
);
console.log(`${out.length} cases (${out.filter((c) => c.rust).length} differ)`);
