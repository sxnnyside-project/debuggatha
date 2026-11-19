/**
 * The client of the daemon: NDJSON over the standard streams of a `debuggatha serve` process.
 *
 * @packageDocumentation
 */

import { type ChildProcess, spawn } from 'node:child_process';
import { createInterface } from 'node:readline';

/** The protocol version this client speaks. */
export const PROTOCOL = 1;

/** What the daemon answered when a request failed: a stable `code`, and a message with no paths. */
export class DaemonError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly errorId?: string,
  ) {
    super(message);
    this.name = 'DaemonError';
  }
}

/** How far a review has got. */
export interface Progress {
  /** The stage the review is in. */
  stage: string;
  /** Work done in the stage. */
  done?: number;
  /** Work the stage has. */
  total?: number;
  /** What the stage is doing. */
  message?: string;
}

/** How to start the daemon. */
export interface Spawn {
  /** The program to run. */
  command: string;
  /** Its arguments. */
  args: readonly string[];
  /** The directory it runs in: the repository. */
  cwd: string;
}

interface Pending {
  resolve(result: unknown): void;
  reject(error: Error): void;
  onProgress?: (progress: Progress) => void;
}

const STDERR_LIMIT = 2_000;
const SHUTDOWN_GRACE_MS = 2_000;

/**
 * One `debuggatha serve` process, spoken to over NDJSON (docs/PROTOCOL.md). Requests run side by
 * side and are answered by id. A process that ends rejects what it still owed, and the client is
 * then finished: whoever holds it starts another.
 */
export class DaemonClient {
  private readonly pending = new Map<string, Pending>();
  private next = 0;
  private closed: Error | undefined;
  private readonly exit: Promise<void>;
  private stderr = '';

  /** What the daemon said about itself in the handshake. */
  server: { name: string; version: string; capabilities: Record<string, unknown> } | undefined;

  private constructor(private readonly child: ChildProcess) {
    this.exit = new Promise((resolve) => child.once('close', () => resolve()));
    child.stderr?.on('data', (chunk: Buffer) => {
      this.stderr = (this.stderr + chunk.toString()).slice(-STDERR_LIMIT);
    });
    const lines = createInterface({ input: child.stdout as NodeJS.ReadableStream });
    lines.on('line', (line) => this.receive(line));
    child.once('error', (error) => this.close(error));
    child.once('close', (code) =>
      this.close(
        new Error(
          `The Debuggatha process ended (code ${code})${this.stderr.trim() ? `: ${this.stderr.trim()}` : '.'}`,
        ),
      ),
    );
  }

  /** Starts the process and shakes hands. Rejects when it cannot start or does not speak protocol 1. */
  static async start(
    spec: Spawn,
    client: { name: string; version: string },
  ): Promise<DaemonClient> {
    const child = spawn(spec.command, [...spec.args], {
      cwd: spec.cwd,
      stdio: ['pipe', 'pipe', 'pipe'],
      windowsHide: true,
    });
    const daemon = new DaemonClient(child);
    try {
      const hello = await daemon.request<{
        server: { name: string; version: string };
        protocol: number;
        capabilities: Record<string, unknown>;
      }>('hello', {
        client,
        protocol: { min: PROTOCOL, max: PROTOCOL },
        capabilities: { progress: true },
      });
      daemon.server = { ...hello.server, capabilities: hello.capabilities };
      return daemon;
    } catch (error) {
      daemon.dispose();
      throw error;
    }
  }

  /** Whether the daemon is still running. */
  get alive(): boolean {
    return this.closed === undefined;
  }

  /**
   * Sends a request and resolves with the daemon's result; `signal` cancels it and `onProgress` hears the stages of a review.
   */
  request<T>(
    method: string,
    params: unknown = {},
    options: { signal?: AbortSignal; onProgress?: (progress: Progress) => void } = {},
  ): Promise<T> {
    if (this.closed) return Promise.reject(this.closed);
    const { signal, onProgress } = options;
    if (signal?.aborted) return Promise.reject(new DaemonError('cancelled', 'Cancelled.'));
    const id = `c-${++this.next}`;
    return new Promise<T>((resolve, reject) => {
      const onAbort = () => this.event('cancel', { target: id });
      this.pending.set(id, {
        resolve: (result) => {
          signal?.removeEventListener('abort', onAbort);
          resolve(result as T);
        },
        reject: (error) => {
          signal?.removeEventListener('abort', onAbort);
          reject(error);
        },
        ...(onProgress ? { onProgress } : {}),
      });
      signal?.addEventListener('abort', onAbort, { once: true });
      this.send({ v: PROTOCOL, type: 'request', id, method, params });
    });
  }

  /** Ends the session: the daemon stops what it runs, including analyzers, and exits. */
  async dispose(): Promise<void> {
    if (!this.closed) {
      this.send({ v: PROTOCOL, type: 'request', id: `c-${++this.next}`, method: 'shutdown' });
      this.child.stdin?.end();
    }
    const timer = setTimeout(() => this.child.kill(), SHUTDOWN_GRACE_MS);
    await this.exit;
    clearTimeout(timer);
  }

  private event(method: string, params: unknown): void {
    this.send({ v: PROTOCOL, type: 'event', method, params });
  }

  private send(message: unknown): void {
    if (this.closed || !this.child.stdin?.writable) return;
    this.child.stdin.write(`${JSON.stringify(message)}\n`);
  }

  private receive(line: string): void {
    let message: {
      type?: string;
      id?: string;
      ok?: boolean;
      method?: string;
      params?: Record<string, unknown>;
      result?: unknown;
      error?: { code?: string; message?: string; errorId?: string };
    };
    try {
      message = JSON.parse(line);
    } catch {
      return;
    }
    if (message.type === 'event' && message.method === 'progress' && message.params) {
      const waiting = this.pending.get(String(message.params.request));
      waiting?.onProgress?.(message.params as unknown as Progress);
      return;
    }
    if (message.type !== 'response' || message.id === undefined) return;
    const waiting = this.pending.get(message.id);
    if (!waiting) return;
    this.pending.delete(message.id);
    if (message.ok) waiting.resolve(message.result);
    else {
      const error = message.error ?? {};
      waiting.reject(
        new DaemonError(
          error.code ?? 'internal',
          error.message ?? 'The request failed.',
          error.errorId,
        ),
      );
    }
  }

  private close(reason: Error): void {
    if (this.closed) return;
    this.closed = reason;
    for (const waiting of this.pending.values()) waiting.reject(reason);
    this.pending.clear();
  }
}
