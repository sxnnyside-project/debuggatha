import { afterEach, describe, expect, it } from 'bun:test';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { ModuleBoundary } from '../types.js';
import { detectEnvDrift } from './env-sync.js';

let dir: string | undefined;

afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true });
  dir = undefined;
});

function repo(files: Record<string, string>): { root: string; boundaries: ModuleBoundary[] } {
  dir = mkdtempSync(join(tmpdir(), 'debuggatha-env-'));
  mkdirSync(join(dir, 'src'), { recursive: true });
  for (const [name, content] of Object.entries(files)) writeFileSync(join(dir, name), content);
  const sources = Object.keys(files).filter((name) => name.startsWith('src/'));
  return { root: dir, boundaries: [{ files: sources } as unknown as ModuleBoundary] };
}

describe('detectEnvDrift', () => {
  it('reports variables the code reads but .env.example does not list', () => {
    const { root, boundaries } = repo({
      '.env.example': 'DATABASE_URL=\n# API_KEY=\n',
      'src/a.ts':
        'const a = process.env.DATABASE_URL;\nconst b = process.env.API_KEY;\nconst c = process.env["STRIPE_KEY"];\n',
    });
    const drift = detectEnvDrift(root, boundaries);
    expect(drift.map((d) => d.variable)).toEqual(['STRIPE_KEY']);
    expect(drift[0]?.locations[0]).toMatchObject({ file: 'src/a.ts', line: 3 });
  });

  it('skips reads with a fallback, platform variables, destructuring defaults and test files', () => {
    const { root, boundaries } = repo({
      '.env.example': 'A=\n',
      'src/a.ts':
        "const p = process.env.PORT ?? '3000';\nconst e = process.env.NODE_ENV;\nconst { X = 1 } = process.env;\n",
      'src/a.test.ts': 'const t = process.env.ONLY_IN_TESTS;\n',
    });
    expect(detectEnvDrift(root, boundaries)).toEqual([]);
  });

  it('reads destructured variables', () => {
    const { root, boundaries } = repo({
      '.env.example': 'A=\n',
      'src/a.ts': 'const { A, B, C: renamed } = process.env;\n',
    });
    expect(detectEnvDrift(root, boundaries).map((d) => d.variable)).toEqual(['B', 'C']);
  });

  it('reports nothing without an example file to compare against', () => {
    const { root, boundaries } = repo({ 'src/a.ts': 'process.env.X;\n' });
    expect(detectEnvDrift(root, boundaries)).toEqual([]);
  });
});
