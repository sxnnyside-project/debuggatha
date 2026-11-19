// Dumps what the TypeScript analysis engine finds (module boundaries, the dependency graph, layers,
// layer violations, cycles, public API, dead code, ownership and the impact of changing each module)
// for every repository of a corpus, for the Rust tests that compare stage by stage.
// Run: bun scripts/parity/dump-analysis.ts <corpus-dir> <out.json>
import { readdirSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { runAnalysis } from '../../packages/engine/src/analysis-engine/index.js';
import { estimateChangeImpact } from '../../packages/engine/src/analysis-engine/stages/change-impact.js';

const [dir, out] = process.argv.slice(2);
if (!dir || !out) {
  console.error('usage: bun scripts/parity/dump-analysis.ts <corpus-dir> <out.json>');
  process.exit(1);
}
const result: Record<string, unknown> = {};
for (const repo of readdirSync(dir).sort()) {
  const root = join(dir, repo);
  if (!statSync(root).isDirectory()) continue;
  const a = runAnalysis(root);
  result[repo] = {
    boundaries: a.moduleBoundaries.map((b) => ({
      id: b.id,
      kind: b.kind,
      name: b.name ?? null,
      files: b.files,
    })),
    nodes: a.dependencyGraph.nodes,
    edges: a.dependencyGraph.edges,
    layers: a.layerModel.assignments,
    violations: a.layerViolations,
    cycles: a.cycles,
    publicApi: a.publicApi,
    deadCode: a.deadCode,
    ownership: a.ownership,
    // What changing every file of one module at a time reaches.
    impact: a.moduleBoundaries.map((b) => {
      const i = estimateChangeImpact(b.files, a.moduleBoundaries, a.dependencyGraph, a.publicApi);
      return {
        module: b.id,
        affected: i.affectedModules,
        downstream: i.downstreamModules,
        publicApi: i.affectedPublicApi.map((s) => `${s.file}:${s.name}`),
        confidence: i.confidence,
      };
    }),
  };
}
writeFileSync(out, JSON.stringify(result));
