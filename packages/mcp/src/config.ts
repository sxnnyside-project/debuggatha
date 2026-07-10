/**
 * Deterministic, transport-specific configuration (epic "Configuration").
 * A pure function of its input env object — no hidden global state, no
 * reading `process.env` deep inside other modules — so tests can pass a
 * fake env instead of mutating the real process environment.
 *
 * Deliberately narrow: "cache location" (the epic's own example) is
 * scoped to what `@debuggatha/repository-intelligence`'s
 * `RepositoryContextCache` actually supports today — an in-memory,
 * per-process cache with no disk backing — so this only exposes an
 * on/off switch, not a path. See README "Technical decisions".
 */

export const LOG_LEVELS = ["debug", "info", "warn", "error"] as const;
export type LogLevel = (typeof LOG_LEVELS)[number];

export interface ServerConfig {
  /** Falls back to a tool's explicit `rootDir` argument when set; used when a tool call omits it. */
  defaultRepositoryRoot: string | undefined;
  cache: { enabled: boolean };
  logLevel: LogLevel;
}

const DEFAULT_CONFIG: ServerConfig = {
  defaultRepositoryRoot: undefined,
  cache: { enabled: true },
  logLevel: "info",
};

function isLogLevel(value: string): value is LogLevel {
  return (LOG_LEVELS as readonly string[]).includes(value);
}

export function loadConfig(env: Record<string, string | undefined> = {}): ServerConfig {
  const logLevelRaw = env.DEBUGGATHA_LOG_LEVEL?.toLowerCase();
  const cacheEnabledRaw = env.DEBUGGATHA_CACHE_ENABLED?.toLowerCase();

  return {
    defaultRepositoryRoot: env.DEBUGGATHA_REPOSITORY_ROOT || undefined,
    cache: {
      enabled:
        cacheEnabledRaw === undefined ? DEFAULT_CONFIG.cache.enabled : cacheEnabledRaw !== "false",
    },
    logLevel: logLevelRaw && isLogLevel(logLevelRaw) ? logLevelRaw : DEFAULT_CONFIG.logLevel,
  };
}

/** Resolves the effective root for a tool call: explicit argument wins, then config default, then a clear error. */
export function resolveRepositoryRoot(
  config: ServerConfig,
  explicitRoot: string | undefined,
): string {
  const root = explicitRoot || config.defaultRepositoryRoot;
  if (!root) {
    throw new Error(
      "No repository root provided — pass `rootDir` in the tool call, or set DEBUGGATHA_REPOSITORY_ROOT.",
    );
  }
  return root;
}
