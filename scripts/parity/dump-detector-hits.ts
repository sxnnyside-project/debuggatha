// Dumps what the TypeScript detectors report for the given rules over every repository of a
// corpus, for the Rust corpus tests that compare detector by detector (no policy needed: the
// golden policies hold only some rules).
// Run: bun scripts/parity/dump-detector-hits.ts <corpus-dir> <out.json> <rule-id>...
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { isGeneratedContent, listScanFiles } from '../../packages/engine/src/scan-scope/index.js';
import { RULE_DETECTORS } from '../../packages/engine/src/skills/engine/detectors.js';
import { lex } from '../../packages/engine/src/skills/engine/lex.js';

const [dir, out, ...rules] = process.argv.slice(2);
if (!dir || !out || rules.length === 0) {
  console.error(
    'usage: bun scripts/parity/dump-detector-hits.ts <corpus-dir> <out.json> <rule-id>...',
  );
  process.exit(1);
}
const EXTENSIONS = /\.(ts|tsx|js|jsx|mjs|cjs|dart|rs|go|py|kt|kts|php|cs|java|vue|svelte)$/;

type Row = [
  repo: string,
  rule: string,
  file: string,
  line: number,
  start: number,
  end: number,
  confidence: string,
  excerpt: string,
  severity: string,
];
const rows: Row[] = [];
for (const repo of readdirSync(dir).sort()) {
  const root = join(dir, repo);
  if (!statSync(root).isDirectory()) continue;
  for (const file of listScanFiles(root, EXTENSIONS)) {
    let content: string;
    try {
      content = readFileSync(join(root, file), 'utf8');
    } catch {
      continue;
    }
    if (isGeneratedContent(content)) continue;
    const lexed = lex(content, file);
    const input = {
      file,
      content,
      lineNumbers: undefined,
      masked: lexed.masked,
      strings: lexed.strings,
    };
    for (const rule of rules) {
      for (const h of RULE_DETECTORS[rule]?.(input) ?? []) {
        rows.push([
          repo,
          rule,
          file,
          h.index + 1,
          h.columns.start,
          h.columns.end,
          h.confidence,
          h.excerpt,
          h.severity ?? '',
        ]);
      }
    }
  }
}
writeFileSync(out, JSON.stringify(rows));
console.log(`${rows.length} hits`);
