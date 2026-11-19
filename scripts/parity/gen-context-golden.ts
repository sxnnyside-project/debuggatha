// Writes corpus/golden/ts-context/<name>.json: the repository context and the policy the
// TypeScript engine derives for every repository of the corpus. The Rust engine must reproduce
// them (phase 2 gate). Run: bun scripts/parity/gen-context-golden.ts <corpus-dir>
//
// <corpus-dir> holds the clones made by `node scripts/parity/fetch-corpus.mjs <corpus-dir>`.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import {
  buildRepositoryContext,
  createInMemoryRegistry,
  resolvePolicy,
} from '../../packages/core/src/index.js';
import { reviewPacks } from '../../packages/packs/src/index.js';

const dir = process.argv[2];
if (!dir) {
  console.error('usage: bun scripts/parity/gen-context-golden.ts <corpus-dir>');
  process.exit(1);
}
const manifest = JSON.parse(readFileSync('corpus/manifest.json', 'utf8'));
const out = 'corpus/golden/ts-context';
mkdirSync(out, { recursive: true });
const registry = createInMemoryRegistry(reviewPacks);
const allPackIds = reviewPacks.map((p) => p.id);

let written = 0;
let failed = 0;
for (const { name } of manifest.repositories as { name: string }[]) {
  const root = join(dir, name);
  if (!existsSync(root)) {
    console.error(`skip ${name}: not cloned`);
    continue;
  }
  let record: unknown;
  try {
    const context = buildRepositoryContext(root);
    const policy = resolvePolicy(context, { requestedPackIds: allPackIds } as never, registry);
    record = {
      name,
      context: { ...JSON.parse(JSON.stringify(context)), rootDir: '<root>' },
      policy: JSON.parse(JSON.stringify(policy)),
    };
  } catch (error) {
    record = { name, error: (error as Error).constructor.name };
    failed += 1;
  }
  writeFileSync(join(out, `${name}.json`), `${JSON.stringify(record)}\n`);
  written += 1;
}
console.log(
  `wrote ${written} files to ${out} (${failed} repositories make the TypeScript scanners throw)`,
);
