/**
 * Deterministic, transport-specific configuration (epic "Configuration").
 * A pure function of its input env object — no hidden global state, no
 * reading `process.env` deep inside other modules — so tests can pass a
 * fake env instead of mutating the real process environment.
 *
 * Deliberately narrow: "cache location" (the epic's own example) is
 * scoped to what `core/repository-intelligence`'s
 * `RepositoryContextCache` actually supports today — an in-memory,
 * per-process cache with no disk backing — so this only exposes an
 * on/off switch, not a path. See README "Technical decisions".
 */

import { ADAPTERS, type AnalyzerOptions, type LocalProviderName } from "@debuggatha/engine";

export const LOG_LEVELS = ["debug", "info", "warn", "error"] as const;
export type LogLevel = (typeof LOG_LEVELS)[number];

export interface ServerConfig {
  /** Falls back to a tool's explicit `rootDir` argument when set; used when a tool call omits it. */
  defaultRepositoryRoot: string | undefined;
  cache: { enabled: boolean };
  logLevel: LogLevel;
  /**
   * Which external analyzers reviews run. Set by whoever starts the server, never by a tool
   * call: an agent may turn analyzers off for a call, but it cannot enable a tool that runs
   * project code or uses the network.
   */
  analyzers: AnalyzerOptions;
  /**
   * Where a model for the semantic pass comes from. Set by whoever starts the server: a tool
   * call can ask for the pass, but cannot choose the model or send code anywhere else.
   */
  semantic: SemanticConfig;
}

export type SemanticProviderName = "off" | "sampling" | LocalProviderName;
export interface SemanticConfig {
  provider: SemanticProviderName;
  model?: string;
  url?: string;
  /** Also attach a model's opinion to uncertain findings. Off unless the operator has measured their model. */
  verify: boolean;
}

const DEFAULT_CONFIG: ServerConfig = {
  defaultRepositoryRoot: undefined,
  cache: { enabled: true },
  logLevel: "info",
  analyzers: { mode: "auto", enable: [] },
  semantic: { provider: "off", verify: false },
};

const SEMANTIC_PROVIDERS: readonly SemanticProviderName[] = [
  "off",
  "sampling",
  "ollama",
  "lmstudio",
];

/**
 * `DEBUGGATHA_SEMANTIC`: `off` (default), `sampling` (the connected client's own model),
 * `ollama`, or `lmstudio` (a local runtime; `DEBUGGATHA_SEMANTIC_MODEL` and `_URL` tune it).
 */
function parseSemantic(env: Record<string, string | undefined>): SemanticConfig {
  const raw = env.DEBUGGATHA_SEMANTIC?.trim().toLowerCase() || "off";
  if (!SEMANTIC_PROVIDERS.includes(raw as SemanticProviderName)) {
    throw new Error(
      `DEBUGGATHA_SEMANTIC must be one of ${SEMANTIC_PROVIDERS.join(", ")}; got "${raw}".`,
    );
  }
  return {
    provider: raw as SemanticProviderName,
    ...(env.DEBUGGATHA_SEMANTIC_MODEL ? { model: env.DEBUGGATHA_SEMANTIC_MODEL } : {}),
    ...(env.DEBUGGATHA_SEMANTIC_URL ? { url: env.DEBUGGATHA_SEMANTIC_URL } : {}),
    verify: env.DEBUGGATHA_SEMANTIC_VERIFY?.toLowerCase() === "true",
  };
}

/**
 * `DEBUGGATHA_ANALYZERS`: `off`, `auto` (the default: tools that only read source), or a
 * comma-separated list of ids to run in addition (`eslint,osv-scanner`).
 */
function parseAnalyzers(raw: string | undefined, timeout: string | undefined): AnalyzerOptions {
  const seconds = Number(timeout);
  const timeoutSeconds = timeout && Number.isFinite(seconds) && seconds > 0 ? seconds : undefined;
  const value = raw?.trim().toLowerCase();
  if (!value || value === "auto") {
    return { mode: "auto", enable: [], ...(timeoutSeconds ? { timeoutSeconds } : {}) };
  }
  if (value === "off") return { mode: "off" };

  const known = ADAPTERS.map((adapter) => adapter.id);
  const enable = value.split(",").map((id) => id.trim());
  const unknown = enable.filter((id) => !known.includes(id));
  if (unknown.length > 0) {
    throw new Error(
      `DEBUGGATHA_ANALYZERS names unknown analyzers: ${unknown.join(", ")}. Known: ${known.join(", ")}, or "auto" / "off".`,
    );
  }
  return { mode: "auto", enable, ...(timeoutSeconds ? { timeoutSeconds } : {}) };
}

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
    analyzers: parseAnalyzers(env.DEBUGGATHA_ANALYZERS, env.DEBUGGATHA_ANALYZER_TIMEOUT),
    semantic: parseSemantic(env),
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
      "No repository root provided — pass `rootDir` in the tool call, set DEBUGGATHA_REPOSITORY_ROOT, or connect from a client that shares a workspace root.",
    );
  }
  return root;
}
