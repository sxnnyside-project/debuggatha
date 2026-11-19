import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

/** The tests drive the real `debuggatha` binary: build it when it is not there. */
export default function setup(): void {
  if (process.env.DEBUGGATHA_CLI) return;
  const root = resolve(import.meta.dirname, '..', '..', '..', '..');
  if (existsSync(resolve(root, 'target', 'debug', 'debuggatha'))) return;
  execFileSync('cargo', ['build', '-p', 'debuggatha-cli', '--locked'], {
    cwd: root,
    stdio: 'inherit',
  });
}
