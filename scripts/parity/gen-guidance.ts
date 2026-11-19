// Writes the guidance texts (why a pattern is a problem, what to change, before/after) as data for
// the Rust engine, from the TypeScript table that is the parity reference (ADR-0006).
// Run: bun scripts/parity/gen-guidance.ts
//
// Once the TypeScript engine is retired this JSON is the source, and it moves into the packs.
import { writeFileSync } from 'node:fs';
import { GUIDANCE } from '../../packages/engine/src/skills/engine/guidance.js';

const sorted = Object.fromEntries(Object.entries(GUIDANCE).sort(([a], [b]) => (a < b ? -1 : 1)));
writeFileSync(
  'crates/debuggatha-engine/data/guidance.json',
  `${JSON.stringify(sorted, null, 2)}\n`,
);
console.log(`${Object.keys(sorted).length} entries`);
