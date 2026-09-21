import { describe, expect, it } from "bun:test";
import { loadConfig, resolveRepositoryRoot } from "./config.js";

describe("loadConfig", () => {
  it("is deterministic and defaults to no repo root, cache enabled, info level", () => {
    expect(loadConfig({})).toEqual({
      defaultRepositoryRoot: undefined,
      cache: { enabled: true },
      logLevel: "info",
      analyzers: { mode: "auto", enable: [] },
      semantic: { provider: "off", verify: false },
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
      analyzers: { mode: "auto", enable: [] },
      semantic: { provider: "off", verify: false },
    });
  });

  describe("DEBUGGATHA_SEMANTIC", () => {
    it("is off unless the operator chooses a model", () => {
      expect(loadConfig({}).semantic).toEqual({ provider: "off", verify: false });
    });

    it("takes the client's own model, or a local runtime with its model and address", () => {
      expect(loadConfig({ DEBUGGATHA_SEMANTIC: "sampling" }).semantic.provider).toBe("sampling");
      expect(
        loadConfig({
          DEBUGGATHA_SEMANTIC: "OLLAMA",
          DEBUGGATHA_SEMANTIC_MODEL: "qwen2.5-coder:7b",
          DEBUGGATHA_SEMANTIC_URL: "http://localhost:11434",
          DEBUGGATHA_SEMANTIC_VERIFY: "true",
        }).semantic,
      ).toEqual({
        provider: "ollama",
        model: "qwen2.5-coder:7b",
        url: "http://localhost:11434",
        verify: true,
      });
    });

    it("refuses a provider it does not know", () => {
      expect(() => loadConfig({ DEBUGGATHA_SEMANTIC: "openai" })).toThrow(/must be one of/);
    });
  });

  describe("DEBUGGATHA_ANALYZERS", () => {
    it("is auto by default and off on request", () => {
      expect(loadConfig({ DEBUGGATHA_ANALYZERS: "auto" }).analyzers).toEqual({
        mode: "auto",
        enable: [],
      });
      expect(loadConfig({ DEBUGGATHA_ANALYZERS: "OFF" }).analyzers).toEqual({ mode: "off" });
    });

    it("enables the analyzers a list names, including those that run project code", () => {
      expect(loadConfig({ DEBUGGATHA_ANALYZERS: "eslint, osv-scanner" }).analyzers).toEqual({
        mode: "auto",
        enable: ["eslint", "osv-scanner"],
      });
    });

    it("refuses a name it does not know rather than silently ignoring a typo", () => {
      expect(() => loadConfig({ DEBUGGATHA_ANALYZERS: "eslnt" })).toThrow(
        /unknown analyzers: eslnt/,
      );
    });

    it("reads the time one analyzer may take", () => {
      expect(loadConfig({ DEBUGGATHA_ANALYZER_TIMEOUT: "45" }).analyzers).toEqual({
        mode: "auto",
        enable: [],
        timeoutSeconds: 45,
      });
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
