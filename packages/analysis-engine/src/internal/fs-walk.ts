import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

export const IGNORED_DIRS = new Set([
  "node_modules",
  ".git",
  "dist",
  "build",
  ".turbo",
  "coverage",
  ".next",
  ".venv",
]);

export const SOURCE_EXTENSIONS = /\.(ts|tsx|js|jsx|mjs|cjs)$/;

/** Every source file under `dir` (relative to `rootDir`), skipping build/dependency directories. */
export function walkSourceFiles(rootDir: string, dir: string = rootDir): string[] {
  const files: string[] = [];
  let entries: string[];
  try {
    entries = readdirSync(dir);
  } catch {
    return files;
  }
  for (const entry of entries) {
    if (IGNORED_DIRS.has(entry) || entry.startsWith(".")) continue;
    const full = join(dir, entry);
    let stat: ReturnType<typeof statSync>;
    try {
      stat = statSync(full);
    } catch {
      continue;
    }
    if (stat.isDirectory()) {
      files.push(...walkSourceFiles(rootDir, full));
    } else if (SOURCE_EXTENSIONS.test(entry)) {
      files.push(relative(rootDir, full));
    }
  }
  return files;
}

export function readJson(path: string): Record<string, unknown> | undefined {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return undefined;
  }
}
