#!/usr/bin/env node
// Fetches every repository in corpus/manifest.json at its pinned commit into <dest>.
// Sources are untrusted data: git hooks are disabled, submodules and LFS are skipped, nothing is executed.
import { execFile } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { promisify } from 'node:util';

const run = promisify(execFile);
const dest = process.argv[2];
if (!dest) {
  console.error('usage: fetch-corpus.mjs <dest-dir> [name...]');
  process.exit(1);
}
const only = new Set(process.argv.slice(3));
const manifest = JSON.parse(
  readFileSync(new URL('../../corpus/manifest.json', import.meta.url), 'utf8'),
);
const env = { ...process.env, GIT_LFS_SKIP_SMUDGE: '1', GIT_TERMINAL_PROMPT: '0' };
const git = (cwd, ...args) => run('git', ['-c', 'core.hooksPath=/dev/null', ...args], { cwd, env });

async function fetchOne({ name, url, commit }) {
  const dir = join(dest, name);
  if (existsSync(join(dir, '.git'))) {
    // An interrupted fetch leaves a `.git` with no commit: that is "not cached", not an error.
    const head = await git(dir, 'rev-parse', 'HEAD').catch(() => null);
    if (head?.stdout.trim() === commit) return 'cached';
  }
  mkdirSync(dir, { recursive: true });
  await git(dir, 'init', '-q');
  await git(
    dir,
    'fetch',
    '-q',
    '--depth',
    '1',
    '--no-tags',
    '--no-recurse-submodules',
    url,
    commit,
  );
  await git(dir, 'checkout', '-q', '--detach', 'FETCH_HEAD');
  return 'fetched';
}

const queue = manifest.repositories.filter((r) => only.size === 0 || only.has(r.name));
let failed = 0;
await Promise.all(
  Array.from({ length: 6 }, async () => {
    for (let repo = queue.shift(); repo; repo = queue.shift()) {
      try {
        console.log(`${await fetchOne(repo)} ${repo.name}`);
      } catch (error) {
        failed += 1;
        console.error(
          `FAILED ${repo.name}: ${String(error.stderr ?? error.message).split('\n')[0]}`,
        );
      }
    }
  }),
);
process.exit(failed ? 1 : 0);
