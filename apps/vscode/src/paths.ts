import { isAbsolute, relative, sep } from 'node:path';

/**
 * The path as the daemon names files: relative to the repository, with forward slashes. A path
 * outside the repository is returned as it was.
 */
export function toRepoRelative(rootDir: string, file: string): string {
  if (!isAbsolute(file)) return file.split(sep).join('/');
  const inside = relative(rootDir, file);
  if (inside === '' || inside.startsWith('..') || isAbsolute(inside)) return file;
  return inside.split(sep).join('/');
}
