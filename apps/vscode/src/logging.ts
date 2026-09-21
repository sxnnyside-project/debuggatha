import type { LogLevel } from "./config/settings.js";

const RANK: Record<LogLevel, number> = { off: 0, error: 1, info: 2, debug: 3 };

export interface LogSink {
  appendLine(line: string): void;
}

/** Longest string a log line carries: enough to diagnose, never enough to hold a file. */
const FIELD_LIMIT = 300;

function show(value: unknown): string {
  if (typeof value === "string") {
    return JSON.stringify(value.length > FIELD_LIMIT ? `${value.slice(0, FIELD_LIMIT)}…` : value);
  }
  if (typeof value === "number" || typeof value === "boolean" || value === undefined) {
    return String(value);
  }
  return show(JSON.stringify(value));
}

/**
 * The extension's log, written to an Output channel. `debuggatha.logLevel` decides what is kept.
 * A line says what happened (which review, how long, which analyzer failed); it never carries
 * the code under review, so a log pasted into an issue does not leak the repository.
 */
export class Logger {
  constructor(
    private readonly sink: LogSink,
    private readonly level: () => LogLevel,
    private readonly now: () => Date = () => new Date(),
  ) {}

  private write(
    level: Exclude<LogLevel, "off">,
    message: string,
    fields?: Record<string, unknown>,
  ) {
    if (RANK[level] > RANK[this.level()]) return;
    const detail = fields
      ? ` ${Object.entries(fields)
          .map(([key, value]) => `${key}=${show(value)}`)
          .join(" ")}`
      : "";
    this.sink.appendLine(
      `${this.now().toISOString()} ${level.toUpperCase().padEnd(5)} ${message}${detail}`,
    );
  }

  error(message: string, fields?: Record<string, unknown>) {
    this.write("error", message, fields);
  }
  info(message: string, fields?: Record<string, unknown>) {
    this.write("info", message, fields);
  }
  debug(message: string, fields?: Record<string, unknown>) {
    this.write("debug", message, fields);
  }
}
