#!/usr/bin/env node
// Runs the CLI (built-in detectors only, nothing persisted) over each fetched corpus repository and writes
// <out>/<name>.json (a stable projection of the findings) plus <out>/summary.json.
// The projection drops random ids so two runs, or two engines, can be compared line by line.
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const [corpusDir, outDir, ...names] = process.argv.slice(2);
if (!corpusDir || !outDir) {
  console.error('usage: run-corpus.mjs <corpus-dir> <out-dir> [name...]');
  process.exit(1);
}
const cli = fileURLToPath(new URL('../../packages/cli/dist/bin.js', import.meta.url));
const manifest = JSON.parse(
  readFileSync(new URL('../../corpus/manifest.json', import.meta.url), 'utf8'),
);
const TIMEOUT_MS = 300_000;
// When set, the raw findings (excerpts included) are also written there, one file per repository.
// They are not committed: the ledger gate regenerates them (see scripts/parity/gen-ledger-golden.ts).
const findingsDir = process.env.DEBUGGATHA_FINDINGS_DIR;
if (findingsDir) mkdirSync(findingsDir, { recursive: true });
mkdirSync(outDir, { recursive: true });

function review(cwd) {
  return new Promise((done) => {
    const started = performance.now();
    const child = spawn(
      process.execPath,
      [
        cli,
        'review',
        '--all',
        '--format',
        'json',
        '--no-persist',
        '--no-analyzers',
        '--fail-on',
        'none',
      ],
      { cwd, stdio: ['ignore', 'pipe', 'pipe'] },
    );
    const out = [];
    const err = [];
    child.stdout.on('data', (c) => out.push(c));
    child.stderr.on('data', (c) => err.push(c));
    const timer = setTimeout(() => child.kill('SIGKILL'), TIMEOUT_MS);
    child.on('close', (code, signal) => {
      clearTimeout(timer);
      done({
        code,
        signal,
        ms: Math.round(performance.now() - started),
        stdout: Buffer.concat(out).toString(),
        stderr: Buffer.concat(err).toString(),
      });
    });
  });
}

const ruleOf = (f) => f.evidence?.find((e) => e.ruleId)?.ruleId ?? null;
const project = (f) => ({
  rule: ruleOf(f),
  pack: f.originatingPack?.id ?? null,
  file: f.locations?.[0]?.file ?? null,
  lines: f.locations?.[0]?.lines ?? null,
  columns: f.locations?.[0]?.columns ?? null,
  severity: f.severity,
  confidence: f.confidence,
  category: f.category,
});
const order = (a, b) =>
  `${a.file}:${a.lines?.start}:${a.rule}`.localeCompare(`${b.file}:${b.lines?.start}:${b.rule}`);

const summary = [];
for (const repo of manifest.repositories) {
  if (names.length && !names.includes(repo.name)) continue;
  const dir = join(corpusDir, repo.name);
  if (!existsSync(dir)) {
    summary.push({ name: repo.name, status: 'missing' });
    continue;
  }
  const result = await review(dir);
  const entry = {
    name: repo.name,
    commit: repo.commit,
    ms: result.ms,
    exit: result.code,
    signal: result.signal,
  };
  try {
    const report = JSON.parse(result.stdout);
    if (findingsDir) {
      writeFileSync(
        join(findingsDir, `${repo.name}.json`),
        JSON.stringify(report.reviewResult.findings),
      );
    }
    const findings = report.reviewResult.findings.map(project).sort(order);
    entry.status = 'ok';
    entry.policy = report.reviewResult.findings[0]?.appliedPolicy?.id ?? null;
    entry.findings = findings.length;
    writeFileSync(
      join(outDir, `${repo.name}.json`),
      `${JSON.stringify({ name: repo.name, commit: repo.commit, findings }, null, 1)}\n`,
    );
  } catch {
    entry.status = result.signal ? 'timeout' : 'error';
    entry.error = result.stderr.split('\n').find(Boolean) ?? result.stdout.slice(0, 200);
  }
  summary.push(entry);
  console.log(
    `${entry.status.padEnd(7)} ${repo.name.padEnd(36)} ${String(entry.findings ?? '-').padStart(6)} findings ${String(entry.ms).padStart(7)} ms`,
  );
}
writeFileSync(join(outDir, 'summary.json'), `${JSON.stringify(summary, null, 1)}\n`);
