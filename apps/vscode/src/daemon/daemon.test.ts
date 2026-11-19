import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, describe, expect, it } from 'vitest';
import { CLI, makeRepo, removeRepos } from '../test/fixtures.js';
import { Daemon } from './api.js';
import { DaemonClient, DaemonError } from './client.js';
import { checkCliVersion } from './compat.js';
import { DaemonConnection, DaemonUnavailable, spawnSpec } from './connection.js';

afterAll(removeRepos);

const settings = (over = {}) => ({
  cliPath: CLI,
  enabledAnalyzers: [] as string[],
  semantic: { provider: 'off' as const },
  trusted: true,
  ...over,
});

describe('checkCliVersion', () => {
  it('accepts the minor line the extension was built against', () => {
    expect(checkCliVersion('0.3.0')).toEqual({ ok: true });
    expect(checkCliVersion('0.3.17')).toEqual({ ok: true });
  });

  it('says to update the CLI when it is older, and the extension when it is newer', () => {
    const older = checkCliVersion('0.2.9');
    const newer = checkCliVersion('0.4.0');
    expect(older.ok === false && older.message).toContain('Update the CLI');
    expect(newer.ok === false && newer.message).toContain('Update the extension');
    expect(checkCliVersion('nope').ok).toBe(false);
  });
});

describe('spawnSpec', () => {
  it('starts the daemon on the workspace with what the person enabled, and only if trusted', () => {
    const on = spawnSpec(
      '/w',
      settings({
        cliPath: '/bin/debuggatha',
        enabledAnalyzers: ['eslint', 'clippy'],
        semantic: { provider: 'ollama', model: 'm', url: 'http://localhost:11434' },
      }),
    );
    expect(on).toEqual({
      command: '/bin/debuggatha',
      args: [
        '--root',
        '/w',
        'serve',
        '--analyzer',
        'eslint',
        'clippy',
        '--semantic',
        'ollama',
        '--semantic-model',
        'm',
        '--semantic-url',
        'http://localhost:11434',
      ],
      cwd: '/w',
    });
    const untrusted = spawnSpec('/w', settings({ enabledAnalyzers: ['eslint'], trusted: false }));
    expect(untrusted.args).not.toContain('--analyzer');
    expect(spawnSpec('/w', settings({ cliPath: undefined })).command).toBe('debuggatha');
  });
});

/** A daemon that speaks the protocol but answers only what a test needs. */
function fakeDaemon(body: string): string {
  const dir = mkdtempSync(join(tmpdir(), 'debuggatha-fake-'));
  const file = join(dir, 'daemon.js');
  writeFileSync(
    file,
    `const rl = require('node:readline').createInterface({ input: process.stdin });
const send = (m) => process.stdout.write(JSON.stringify({ v: 1, ...m }) + '\\n');
const hello = (id, version) => send({ type: 'response', id, ok: true, result: {
  server: { name: 'debuggatha', version }, protocol: 1, root: '.', capabilities: {} } });
rl.on('line', (line) => {
  const m = JSON.parse(line);
  ${body}
});`,
  );
  return file;
}

const viaNode = (script: string, version = '0.3.0') => ({
  command: process.execPath,
  args: [script, version],
  cwd: tmpdir(),
});

describe('DaemonClient', () => {
  const client = { name: 'test', version: '0' };

  it('shakes hands, answers by id, and stops a review that is cancelled', async () => {
    const script = fakeDaemon(`
      if (m.method === 'hello') return hello(m.id, process.argv[2]);
      if (m.method === 'review/run') {
        send({ type: 'event', method: 'progress', params: { request: m.id, stage: 'detect', done: 1, total: 2 } });
        return;
      }
      if (m.type === 'event' && m.method === 'cancel')
        return send({ type: 'response', id: m.params.target, ok: false, error: { code: 'cancelled', message: 'Cancelled.' } });
      if (m.method === 'shutdown') process.exit(0);
    `);
    const daemon = await DaemonClient.start(viaNode(script), client);
    expect(daemon.server?.version).toBe('0.3.0');

    const controller = new AbortController();
    const stages: string[] = [];
    const review = daemon.request(
      'review/run',
      {},
      {
        signal: controller.signal,
        onProgress: (p) => {
          stages.push(p.stage);
          controller.abort();
        },
      },
    );
    await expect(review).rejects.toMatchObject({ code: 'cancelled' });
    expect(stages).toEqual(['detect']);
    await daemon.dispose();
    expect(daemon.alive).toBe(false);
    rmSync(script, { force: true });
  });

  it('rejects what a daemon that ended still owed, with what it said, and is then not alive', async () => {
    const script = fakeDaemon(`
      if (m.method === 'hello') return hello(m.id, '0.3.0');
      console.error('boom'); process.exit(3);
    `);
    const daemon = await DaemonClient.start(viaNode(script), client);
    await expect(daemon.request('findings/list')).rejects.toThrow(/ended \(code 3\).*boom/);
    expect(daemon.alive).toBe(false);
    await expect(daemon.request('findings/list')).rejects.toThrow();
  });

  it('keeps the daemon error code and message, nothing else', async () => {
    const script = fakeDaemon(`
      if (m.method === 'hello') return hello(m.id, '0.3.0');
      send({ type: 'response', id: m.id, ok: false, error: { code: 'not_found', message: 'No finding has that id.', errorId: 'e-1' } });
    `);
    const daemon = await DaemonClient.start(viaNode(script), client);
    const error = await daemon.request('findings/get', { id: 'x' }).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(DaemonError);
    expect(error).toMatchObject({ code: 'not_found', errorId: 'e-1' });
    await daemon.dispose();
  });
});

describe('DaemonConnection', () => {
  const options = (root: string | undefined, current: () => ReturnType<typeof settings>) => ({
    rootDir: () => root,
    settings: current,
    client: { name: 'test', version: '0' },
  });

  it('starts once, reuses the daemon, and starts another when a setting that decides it changes', async () => {
    const { root } = makeRepo();
    let analyzers: string[] = [];
    const connection = new DaemonConnection(
      options(root, () => settings({ enabledAnalyzers: analyzers })),
    );
    const first = await connection.get();
    expect(await connection.get()).toBe(first);
    analyzers = ['eslint'];
    const second = await connection.get();
    expect(second).not.toBe(first);
    expect(second.server?.capabilities.enabledAnalyzers).toEqual(['eslint']);
    await connection.dispose();
  });

  it('starts again after the daemon ended', async () => {
    const { root } = makeRepo();
    const connection = new DaemonConnection(options(root, () => settings()));
    const first = await connection.get();
    await first.dispose();
    const second = await connection.get();
    expect(second).not.toBe(first);
    expect(second.alive).toBe(true);
    await connection.dispose();
  });

  it('names the setting when the binary is missing, and never repeats the path', async () => {
    const { root } = makeRepo();
    const connection = new DaemonConnection(
      options(root, () => settings({ cliPath: '/nonexistent/dir/debuggatha' })),
    );
    const error = await connection.get().catch((e: unknown) => e);
    expect(error).toBeInstanceOf(DaemonUnavailable);
    expect((error as DaemonUnavailable).setting).toBe(true);
    expect((error as Error).message).toContain('debuggatha.cliPath');
    expect((error as Error).message).not.toContain('/nonexistent');
    // A failed start is not remembered.
    await expect(connection.get()).rejects.toBeInstanceOf(DaemonUnavailable);
  });

  it('refuses a binary of another version, and says which side to update', async () => {
    const script = fakeDaemon(`if (m.method === 'hello') hello(m.id, process.argv[2]);`);
    const { root } = makeRepo();
    const connection = new DaemonConnection({
      ...options(root, () => settings()),
      start: (_spec, client) => DaemonClient.start(viaNode(script, '0.9.0'), client),
    });
    await expect(connection.get()).rejects.toThrow(/Update the extension/);
  });

  it('needs a workspace', async () => {
    const connection = new DaemonConnection(options(undefined, () => settings()));
    await expect(connection.get()).rejects.toThrow('No workspace folder open.');
  });
});

describe('Daemon (the real binary)', () => {
  it('reviews, reads the ledger back, moves a finding, and describes the repository', async () => {
    const { root } = makeRepo();
    const connection = new DaemonConnection({
      rootDir: () => root,
      settings: () => settings(),
      client: { name: 'test', version: '0' },
    });
    const daemon = new Daemon(connection);

    const stages = new Set<string>();
    const result = await daemon.review(
      { scope: { kind: 'workspace' }, packIds: [], analyzers: { mode: 'off' }, semantic: false },
      new AbortController().signal,
      (p) => stages.add(p.stage),
    );
    expect(result.findings).toBeGreaterThan(0);
    expect(result.coverage).toBeDefined();
    expect(stages.size).toBeGreaterThan(0);

    const entries = await daemon.ledger();
    expect(entries).toHaveLength(result.findings);
    const first = entries[0];
    if (!first) throw new Error('no entries');
    expect((await daemon.setStatus(first.id, 'dismissed')).status).toBe('dismissed');
    await expect(daemon.setStatus(first.id, 'acknowledged')).rejects.toMatchObject({
      code: 'conflict',
    });
    expect(await daemon.inlineComment(first.id)).toContain('debuggatha-ignore-next-line');
    expect((await daemon.context()).stack.languages.length).toBeGreaterThan(0);
    expect((await daemon.policy([])).packRefs.length).toBeGreaterThan(0);
    await connection.dispose();
  });
});
