// Packs the npm launchers and this machine's platform package, installs the tarballs into an empty
// project, and runs what a user would run: `debuggatha --version` and an MCP handshake, under Node
// and under Bun. DEBUGGATHA_BINARY names the release binary to ship in the platform package.
import { execFileSync, spawn } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const root = resolve(import.meta.dirname, '..');
const binary = process.env.DEBUGGATHA_BINARY ?? join(root, 'target', 'release', 'debuggatha');
const platform = `${process.platform}-${process.arch}`;
const run = (cmd, args, cwd) => execFileSync(cmd, args, { cwd, encoding: 'utf8' }).trim();
const version = JSON.parse(readFileSync(join(root, 'npm/cli/package.json'), 'utf8')).version;

/** Speaks MCP over stdio to the installed server and lists its tools. */
async function handshake(runtime, script, cwd) {
  const child = spawn(runtime, [script], { cwd, stdio: ['pipe', 'pipe', 'ignore'] });
  const pending = new Map();
  let buffer = '';
  child.stdout.on('data', (chunk) => {
    buffer += chunk;
    for (let i = buffer.indexOf('\n'); i >= 0; i = buffer.indexOf('\n')) {
      const line = buffer.slice(0, i);
      buffer = buffer.slice(i + 1);
      try {
        const message = JSON.parse(line);
        pending.get(message.id)?.(message);
      } catch {
        // non-JSON output is ignored
      }
    }
  });
  let nextId = 0;
  const rpc = (method, params) =>
    new Promise((resolveRpc, reject) => {
      const id = ++nextId;
      const timer = setTimeout(() => reject(new Error(`${method} timed out`)), 15_000);
      pending.set(id, (message) => {
        clearTimeout(timer);
        resolveRpc(message);
      });
      child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
    });
  try {
    const init = await rpc('initialize', {
      protocolVersion: '2025-06-18',
      capabilities: {},
      clientInfo: { name: 'smoke', version: '0' },
    });
    child.stdin.write(
      `${JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' })}\n`,
    );
    const tools = await rpc('tools/list', {});
    return {
      version: init.result?.serverInfo?.version,
      tools: tools.result?.tools?.map((tool) => tool.name) ?? [],
    };
  } finally {
    child.kill();
  }
}

const work = mkdtempSync(join(tmpdir(), 'debuggatha-smoke-'));
try {
  const tarballs = join(work, 'tarballs');
  const app = join(work, 'app');
  mkdirSync(tarballs);
  mkdirSync(app);
  const platformDir = run(
    join(root, 'scripts/package-npm.sh'),
    [platform, binary, version, join(work, 'platform')],
    root,
  );
  const packed = [platformDir, join(root, 'npm/cli'), join(root, 'npm/mcp')].map((dir) =>
    join(tarballs, run('npm', ['pack', '--silent', '--pack-destination', tarballs], dir)),
  );
  run('npm', ['init', '-y'], app);
  run('npm', ['install', '--silent', '--no-audit', '--no-fund', ...packed], app);

  const installed = (pkg) => join(app, 'node_modules', '@sxnnyside', `debuggatha-${pkg}`, 'bin.js');
  let failed = false;
  for (const runtime of ['node', 'bun']) {
    const printed = run(runtime, [installed('cli'), '--version'], app);
    if (printed !== `debuggatha ${version}`) {
      console.error(`smoke: [${runtime}] --version printed "${printed}"`);
      failed = true;
    } else console.log(`ok  [${runtime}] ${printed}`);
    const mcp = await handshake(runtime, installed('mcp'), app);
    if (mcp.version !== version || mcp.tools.length === 0) {
      console.error(`smoke: [${runtime}] MCP answered ${JSON.stringify(mcp)}`);
      failed = true;
    } else console.log(`ok  [${runtime}] debuggatha-mcp handshake -> ${mcp.tools.length} tools`);
  }
  if (failed) process.exitCode = 1;
} finally {
  rmSync(work, { recursive: true, force: true });
}
