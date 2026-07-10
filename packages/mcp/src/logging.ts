import type { LogLevel } from "./config.js";

/**
 * Structured logging (epic "Logging") — focused on tool execution,
 * duration, failures, and transport events, per the epic's own list.
 *
 * CRITICAL: this must never write to stdout. A stdio-transport MCP
 * server uses stdout exclusively for the JSON-RPC protocol stream —
 * anything else written there corrupts it. Every log line goes to
 * stderr instead (see README "Technical decisions").
 *
 * "Avoid logging sensitive repository contents" — callers pass small,
 * pre-shaped `data` objects (tool name, duration, counts, ids), never a
 * whole `RepositoryContext`, `Finding`, or file excerpt. This module
 * doesn't enforce that (it can't know what a caller passes), so tool
 * handlers are responsible for only logging metadata — see `tools/*`.
 */

const LEVEL_RANK: Record<LogLevel, number> = { debug: 0, info: 1, warn: 2, error: 3 };

export interface LogEvent {
  timestamp: string;
  level: LogLevel;
  event: string;
  data: Record<string, unknown>;
}

export interface Logger {
  log(level: LogLevel, event: string, data?: Record<string, unknown>): void;
  debug(event: string, data?: Record<string, unknown>): void;
  info(event: string, data?: Record<string, unknown>): void;
  warn(event: string, data?: Record<string, unknown>): void;
  error(event: string, data?: Record<string, unknown>): void;
}

export interface CreateLoggerOptions {
  minLevel: LogLevel;
  /** Injectable sink for testing — defaults to `process.stderr`. Never point this at stdout. */
  write?: (line: string) => void;
}

export function createLogger(options: CreateLoggerOptions): Logger {
  const write = options.write ?? ((line: string) => process.stderr.write(`${line}\n`));

  const log = (level: LogLevel, event: string, data: Record<string, unknown> = {}): void => {
    if (LEVEL_RANK[level] < LEVEL_RANK[options.minLevel]) return;
    const entry: LogEvent = { timestamp: new Date().toISOString(), level, event, data };
    write(JSON.stringify(entry));
  };

  return {
    log,
    debug: (event, data) => log("debug", event, data),
    info: (event, data) => log("info", event, data),
    warn: (event, data) => log("warn", event, data),
    error: (event, data) => log("error", event, data),
  };
}
