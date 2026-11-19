import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { isTestPath } from '../../scan-scope/index.js';
import type { EnvDrift, ModuleBoundary } from '../types.js';

const EXAMPLE_FILES = ['.env.example', '.env.sample', '.env.template'];
const JS_LIKE = /\.(ts|tsx|js|jsx|mjs|cjs)$/;
/** Set by the platform or the shell, not something an `.env.example` documents. */
const PLATFORM_VARS = new Set(
  'NODE_ENV CI HOME PATH PWD USER SHELL TZ TERM LANG TMPDIR FORCE_COLOR NO_COLOR'.split(' '),
);
const ENV_ACCESS = /process\.env(?:\.([A-Za-z_]\w*)|\[\s*["']([A-Za-z_]\w*)["']\s*\])/g;
const MAX_LOCATIONS = 3;

/**
 * Environment drift: a variable the code reads from `process.env` that the
 * repository's `.env.example` does not list. Only runs when such a file
 * exists (without it there is nothing to compare against). Reads with a
 * fallback (`?? 'x'`, `|| 'x'`) are optional by construction and skipped.
 *
 * shortcut: only `process.env.X` / `process.env['X']` / single-line
 * destructuring, JS/TS files; `import.meta.env` and other languages are not
 * read. Upgrade when the AST layer lands.
 */
export function detectEnvDrift(rootDir: string, boundaries: ModuleBoundary[]): EnvDrift[] {
  const exampleFile = EXAMPLE_FILES.find((name) => existsSync(join(rootDir, name)));
  if (!exampleFile) return [];

  const declared = new Set<string>();
  try {
    for (const line of readFileSync(join(rootDir, exampleFile), 'utf8').split('\n')) {
      const name = /^\s*(?:export\s+)?#?\s*([A-Za-z_][A-Za-z0-9_]*)\s*=/.exec(line)?.[1];
      if (name) declared.add(name);
    }
  } catch {
    return [];
  }

  const byVariable = new Map<string, EnvDrift>();
  const record = (variable: string, file: string, line: number, excerpt: string) => {
    if (declared.has(variable) || PLATFORM_VARS.has(variable)) return;
    const drift = byVariable.get(variable) ?? { variable, exampleFile, locations: [] };
    byVariable.set(variable, drift);
    if (drift.locations.length < MAX_LOCATIONS) {
      drift.locations.push({ file, line, excerpt: excerpt.trim().slice(0, 200) });
    }
  };

  for (const file of boundaries.flatMap((boundary) => boundary.files)) {
    if (!JS_LIKE.test(file) || isTestPath(file)) continue;
    let content: string;
    try {
      content = readFileSync(join(rootDir, file), 'utf8');
    } catch {
      continue;
    }
    content.split('\n').forEach((text, index) => {
      if (/^\s*(?:\/\/|\*|\/\*)/.test(text)) return;
      for (const match of text.matchAll(ENV_ACCESS)) {
        const rest = text.slice((match.index ?? 0) + match[0].length);
        if (/^\s*(?:\?\?|\|\|)/.test(rest)) continue;
        record(match[1] ?? match[2] ?? '', file, index + 1, text);
      }
      const destructured = /\{([^}]+)\}\s*=\s*process\.env\b/.exec(text)?.[1];
      for (const part of destructured?.split(',') ?? []) {
        if (part.includes('=')) continue;
        const name = part.split(':')[0]?.trim();
        if (name && /^[A-Za-z_]\w*$/.test(name)) record(name, file, index + 1, text);
      }
    });
  }
  return [...byVariable.values()].sort((a, b) => a.variable.localeCompare(b.variable));
}
