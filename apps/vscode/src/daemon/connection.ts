/**
 * The one daemon the extension keeps: started on first use, and started again when it ended or when a setting that decides how it starts changed.
 *
 * @packageDocumentation
 */

import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { DaemonClient, type Progress, type Spawn } from './client.js';
import { checkCliVersion } from './compat.js';

/** What decides how the daemon is started: everything a request is not allowed to change. */
export interface ConnectionSettings {
  /** Absolute path of the binary; empty means `debuggatha` on the PATH. Machine scope. */
  cliPath: string | undefined;
  /** Analyzers that run project code or use the network, which only the person can enable. */
  enabledAnalyzers: readonly string[];
  /** The model on this machine for the semantic pass; `off` sends nothing anywhere. */
  semantic: { provider: 'off' | 'ollama' | 'lmstudio'; model?: string; url?: string };
  /** An untrusted workspace gets the built-in detectors only. */
  trusted: boolean;
}

/** What a connection needs from the extension. */
export interface ConnectionOptions {
  /** The repository root, or `undefined` when no folder is open. */
  rootDir(): string | undefined;
  /** The settings that decide how the daemon starts. */
  settings(): ConnectionSettings;
  /** Who is connecting, for the handshake. */
  client: { name: string; version: string };
  /** Replaced by tests. */
  start?: typeof DaemonClient.start;
  /** Replaced by tests: where to look when the PATH has no binary. */
  locations?: readonly string[];
}

/**
 * Where the binary usually lands when it is installed but its folder is not on the PATH of the
 * editor (an editor launched from the dock does not read the shell's profile).
 */
export function usualLocations(home: string, platform: NodeJS.Platform): string[] {
  const exe = platform === 'win32' ? 'debuggatha.exe' : 'debuggatha';
  const folders =
    platform === 'win32'
      ? [join(home, '.cargo', 'bin')]
      : [
          join(home, '.cargo', 'bin'),
          '/opt/homebrew/bin',
          '/usr/local/bin',
          join(home, '.local', 'bin'),
        ];
  return folders.map((folder) => join(folder, exe));
}

/** A failure the person can act on, worded for them. */
export class DaemonUnavailable extends Error {
  constructor(
    message: string,
    /** True when the fix is a setting (the path of the binary). */
    readonly setting = false,
  ) {
    super(message);
    this.name = 'DaemonUnavailable';
  }
}

/**
 * How to start the daemon for `rootDir`: only what the person's own settings allow, and no analyzer in an untrusted workspace.
 */
export function spawnSpec(rootDir: string, settings: ConnectionSettings): Spawn {
  const args = ['--root', rootDir, 'serve'];
  const analyzers = settings.trusted ? settings.enabledAnalyzers : [];
  if (analyzers.length > 0) args.push('--analyzer', ...analyzers);
  const { semantic } = settings;
  if (semantic.provider !== 'off') {
    args.push('--semantic', semantic.provider);
    if (semantic.model) args.push('--semantic-model', semantic.model);
    if (semantic.url) args.push('--semantic-url', semantic.url);
  }
  return { command: settings.cliPath?.trim() || 'debuggatha', args, cwd: rootDir };
}

/**
 * The extension's one daemon: started on first use, started again when it has ended or when a
 * setting that decides how it starts has changed (an analyzer, the model, trust, the binary).
 */
export class DaemonConnection {
  private current: { key: string; client: Promise<DaemonClient> } | undefined;

  constructor(private readonly options: ConnectionOptions) {}

  /** The running daemon, started if there is none or the one there ended or is stale. */
  async get(): Promise<DaemonClient> {
    const rootDir = this.options.rootDir();
    if (!rootDir) throw new DaemonUnavailable('No workspace folder open.');
    const spec = spawnSpec(rootDir, this.options.settings());
    const key = JSON.stringify(spec);

    if (this.current && this.current.key !== key) {
      const stale = this.current.client;
      this.current = undefined;
      void stale.then((client) => client.dispose()).catch(() => {});
    }
    if (this.current && !(await this.alive(this.current.client))) this.current = undefined;
    if (!this.current) {
      this.current = { key, client: this.launch(spec) };
      // A failed start must not be remembered: the next request tries again.
      this.current.client.catch(() => {
        if (this.current?.key === key) this.current = undefined;
      });
    }
    return this.current.client;
  }

  /** Sends a request to the daemon, starting it if needed. */
  async request<T>(
    method: string,
    params: unknown = {},
    options?: { signal?: AbortSignal; onProgress?: (progress: Progress) => void },
  ): Promise<T> {
    return (await this.get()).request<T>(method, params, options);
  }

  /** Stops the daemon. */
  async dispose(): Promise<void> {
    const current = this.current;
    this.current = undefined;
    await current?.client.then((client) => client.dispose()).catch(() => {});
  }

  /** The binary in a usual place, for a PATH that does not have it. */
  private installed(): string | undefined {
    return (this.options.locations ?? usualLocations(homedir(), process.platform)).find((path) =>
      existsSync(path),
    );
  }

  private async alive(client: Promise<DaemonClient>): Promise<boolean> {
    try {
      return (await client).alive;
    } catch {
      return false;
    }
  }

  private async launch(spec: Spawn): Promise<DaemonClient> {
    const start = this.options.start ?? DaemonClient.start;
    let client: DaemonClient;
    try {
      client = await start(spec, this.options.client).catch((error: unknown) => {
        const missing = (error as NodeJS.ErrnoException).code === 'ENOENT';
        const found = missing && spec.command === 'debuggatha' ? this.installed() : undefined;
        if (!found) throw error;
        return start({ ...spec, command: found }, this.options.client);
      });
    } catch (error) {
      const code = (error as NodeJS.ErrnoException).code;
      if (code === 'ENOENT' || code === 'EACCES') {
        throw new DaemonUnavailable(
          spec.command === 'debuggatha'
            ? 'The Debuggatha CLI was not found on the PATH or in its usual places. Install it, or set debuggatha.cliPath to the binary.'
            : 'debuggatha.cliPath does not point to a Debuggatha binary this machine can run.',
          true,
        );
      }
      if (error instanceof Error && 'code' in error && error.code === 'protocol_mismatch') {
        throw new DaemonUnavailable(
          'The Debuggatha CLI speaks another protocol than this extension. Update the older of the two.',
        );
      }
      throw error;
    }
    const version = client.server?.version ?? '';
    const compatible = checkCliVersion(version);
    if (!compatible.ok) {
      void client.dispose();
      throw new DaemonUnavailable(compatible.message);
    }
    return client;
  }
}
