import { describe, expect, it } from "vitest";
import { loadConfig, resolveRepositoryRoot } from "./config.js";

describe("loadConfig", () => {
  it("is deterministic and defaults to no repo root, cache enabled, info level", () => {
    expect(loadConfig({})).toEqual({
      defaultRepositoryRoot: undefined,
      cache: { enabled: true },
      logLevel: "info",
    });
  });

  it("reads DEBUGGATHA_REPOSITORY_ROOT, DEBUGGATHA_CACHE_ENABLED, DEBUGGATHA_LOG_LEVEL", () => {
    const config = loadConfig({
      DEBUGGATHA_REPOSITORY_ROOT: "/repo",
      DEBUGGATHA_CACHE_ENABLED: "false",
      DEBUGGATHA_LOG_LEVEL: "DEBUG",
    });

    expect(config).toEqual({
      defaultRepositoryRoot: "/repo",
      cache: { enabled: false },
      logLevel: "debug",
    });
  });

  it("falls back to the default log level for an unrecognized value instead of throwing", () => {
    expect(loadConfig({ DEBUGGATHA_LOG_LEVEL: "verbose" }).logLevel).toBe("info");
  });

  it("never mutates or reads the real process.env — same input always produces the same output", () => {
    const env = { DEBUGGATHA_REPOSITORY_ROOT: "/a" };
    expect(loadConfig(env)).toEqual(loadConfig(env));
  });
});

describe("resolveRepositoryRoot", () => {
  it("prefers an explicit argument over the configured default", () => {
    const config = loadConfig({ DEBUGGATHA_REPOSITORY_ROOT: "/default" });
    expect(resolveRepositoryRoot(config, "/explicit")).toBe("/explicit");
  });

  it("falls back to the configured default when no explicit argument is given", () => {
    const config = loadConfig({ DEBUGGATHA_REPOSITORY_ROOT: "/default" });
    expect(resolveRepositoryRoot(config, undefined)).toBe("/default");
  });

  it("throws a clear error when neither is available", () => {
    const config = loadConfig({});
    expect(() => resolveRepositoryRoot(config, undefined)).toThrow(/No repository root/);
  });
});
