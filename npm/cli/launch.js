// Runs the native `debuggatha` binary with this process's terminal and exit code. The package
// carries no engine: the binary comes from the package of the platform (an optional dependency),
// from DEBUGGATHA_CLI, or from PATH.
import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';

const PLATFORMS = new Set(['darwin-arm64', 'linux-x64', 'linux-arm64', 'win32-x64']);

function binary() {
  if (process.env.DEBUGGATHA_CLI) return process.env.DEBUGGATHA_CLI;
  const platform = `${process.platform}-${process.arch}`;
  if (PLATFORMS.has(platform)) {
    const exe = process.platform === 'win32' ? 'debuggatha.exe' : 'debuggatha';
    try {
      return createRequire(import.meta.url).resolve(`@sxnnyside/debuggatha-${platform}/bin/${exe}`);
    } catch {
      // not installed (optional dependencies were skipped): fall through to PATH
    }
  }
  return 'debuggatha';
}

/** `prefix` goes before the arguments of this process (`['mcp']` for the MCP server). */
export function launch(prefix) {
  const child = spawn(binary(), [...prefix, ...process.argv.slice(2)], { stdio: 'inherit' });
  child.on('error', (error) => {
    console.error(
      error.code === 'ENOENT'
        ? 'debuggatha: the native binary was not found. Reinstall without --no-optional, install it with brew or cargo install debuggatha-cli, or set DEBUGGATHA_CLI to its path.'
        : `debuggatha: could not start the binary (${error.code ?? 'error'}).`,
    );
    process.exit(1);
  });
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal));
  child.on('exit', (code, signal) => process.exit(code ?? (signal ? 1 : 0)));
}
