import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import type { Finding, LedgerEntry } from '../contracts/index.js';

export type { Finding, LedgerEntry };

/** The binary the tests drive: `cargo build -p debuggatha-cli` makes it (the global setup does). */
export const CLI =
  process.env.DEBUGGATHA_CLI ??
  resolve(import.meta.dirname, '..', '..', '..', '..', 'target', 'debug', 'debuggatha');

/** A finding as a test needs one: the fields a detector would always give, the rest defaulted. */
export function createFinding(input: Record<string, unknown>): Finding {
  return { id: randomUUID(), recommendations: [], ...input } as unknown as Finding;
}

/** What the daemon wrote to `.debuggatha/ledger.json`: the persisted, versioned file. */
export function loadLedger(root: string): { entries: LedgerEntry[] } {
  try {
    return JSON.parse(readFileSync(join(root, '.debuggatha', 'ledger.json'), 'utf8'));
  } catch {
    return { entries: [] };
  }
}

/** Source that the detected packs flag: `any`, `==`, `eval`, and a hardcoded secret. */
export const BAD_SOURCE =
  'var x: any = 1;\nif (x == 2) { eval("1"); }\nconst password = "hunter22222";\n';

const created: string[] = [];

export function makeRepo(source = BAD_SOURCE): { root: string; badFile: string } {
  const root = mkdtempSync(join(tmpdir(), 'debuggatha-vscode-'));
  created.push(root);
  mkdirSync(join(root, 'src'));
  writeFileSync(
    join(root, 'package.json'),
    JSON.stringify({ name: 'fixture', devDependencies: { typescript: '^5.0.0' } }),
  );
  const badFile = join(root, 'src', 'bad.ts');
  writeFileSync(badFile, source);
  return { root, badFile };
}

/** Runs a real workspace review with the binary, so the repository has a ledger with genuine entries. */
export function seedLedger(root: string): void {
  execFileSync(CLI, ['--root', root, 'review', '--all', '--fail-on', 'none', '--quiet'], {
    stdio: 'ignore',
  });
}

export function git(cwd: string, ...args: string[]): void {
  execFileSync(
    'git',
    ['-c', 'user.name=t', '-c', 'user.email=t@t', '-c', 'commit.gpgsign=false', ...args],
    { cwd, stdio: 'ignore' },
  );
}

export function removeRepos(): void {
  for (const dir of created.splice(0)) rmSync(dir, { recursive: true, force: true });
}
