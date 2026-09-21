import { describe, expect, it, vi } from "bun:test";
import { createRuntimeEngine, RuntimeSelectionError } from "./engine.js";
import type { Provider } from "./provider.js";
import { createProviderRegistry } from "./registry.js";
import type { SemanticStreamEvent } from "./types.js";

function mockProvider(overrides: Partial<Provider> & { id: string }): Provider {
  return {
    displayName: overrides.id,
    discoverModels: vi
      .fn()
      .mockResolvedValue([{ id: "model-a", provider: overrides.id, contextWindow: 8192 }]),
    healthCheck: vi.fn().mockResolvedValue({ available: true, detail: "ok" }),
    contextWindowFor: vi.fn().mockResolvedValue(8192),
    execute: async function* (): AsyncIterable<SemanticStreamEvent> {
      yield { kind: "token", text: '{"candidates":[]}' };
      yield { kind: "done" };
    },
    ...overrides,
  };
}

describe("resolveRuntime (Runtime Selection)", () => {
  it("automatic mode picks the first healthy provider and its first model", async () => {
    const unhealthy = mockProvider({
      id: "unhealthy",
      healthCheck: vi.fn().mockResolvedValue({ available: false, detail: "down" }),
    });
    const healthy = mockProvider({ id: "healthy" });
    const engine = createRuntimeEngine(createProviderRegistry([unhealthy, healthy]));

    const resolved = await engine.resolveRuntime({ mode: "automatic" });
    expect(resolved.provider.id).toBe("healthy");
    expect(resolved.model).toBe("model-a");
  });

  it("automatic mode throws naming providers checked when none are healthy", async () => {
    const engine = createRuntimeEngine(
      createProviderRegistry([
        mockProvider({
          id: "a",
          healthCheck: vi.fn().mockResolvedValue({ available: false, detail: "" }),
        }),
      ]),
    );
    await expect(engine.resolveRuntime({ mode: "automatic" })).rejects.toThrow(
      RuntimeSelectionError,
    );
  });

  it("explicit provider mode resolves by id, defaulting to its first model", async () => {
    const engine = createRuntimeEngine(createProviderRegistry([mockProvider({ id: "ollama" })]));
    const resolved = await engine.resolveRuntime({
      mode: "provider",
      providerId: "ollama",
      model: undefined,
    });
    expect(resolved.provider.id).toBe("ollama");
    expect(resolved.model).toBe("model-a");
  });

  it("explicit provider mode honors an explicit model without calling discoverModels", async () => {
    const provider = mockProvider({ id: "ollama" });
    const engine = createRuntimeEngine(createProviderRegistry([provider]));
    const resolved = await engine.resolveRuntime({
      mode: "provider",
      providerId: "ollama",
      model: "custom-model",
    });
    expect(resolved.model).toBe("custom-model");
  });

  it("explicit provider mode throws for an unregistered provider id", async () => {
    const engine = createRuntimeEngine(createProviderRegistry([]));
    await expect(
      engine.resolveRuntime({ mode: "provider", providerId: "ghost", model: undefined }),
    ).rejects.toThrow(/ghost/);
  });

  it("explicit model mode finds whichever provider serves that model", async () => {
    const a = mockProvider({
      id: "a",
      discoverModels: vi
        .fn()
        .mockResolvedValue([{ id: "x", provider: "a", contextWindow: undefined }]),
    });
    const b = mockProvider({
      id: "b",
      discoverModels: vi
        .fn()
        .mockResolvedValue([{ id: "y", provider: "b", contextWindow: undefined }]),
    });
    const engine = createRuntimeEngine(createProviderRegistry([a, b]));

    const resolved = await engine.resolveRuntime({ mode: "model", model: "y" });
    expect(resolved.provider.id).toBe("b");
  });

  it("explicit model mode throws when no provider serves it", async () => {
    const engine = createRuntimeEngine(createProviderRegistry([mockProvider({ id: "a" })]));
    await expect(engine.resolveRuntime({ mode: "model", model: "nope" })).rejects.toThrow(/nope/);
  });
});

describe("runSemanticReview", () => {
  it("runs end-to-end and parses the accumulated stream into a result", async () => {
    const provider = mockProvider({
      id: "ollama",
      execute: async function* (): AsyncIterable<SemanticStreamEvent> {
        yield { kind: "token", text: '{"candidates":[{"title":"t","explanation":"e",' };
        yield {
          kind: "token",
          text: '"confidence":"high","evidence":[{"file":"a.ts","excerpt":"x"}]}]}',
        };
        yield { kind: "done" };
      },
    });
    const engine = createRuntimeEngine(createProviderRegistry([provider]));

    const { result, metadata } = await engine.runSemanticReview({
      units: [{ file: "a.ts", content: "const x = 1;" }],
      selection: { mode: "automatic" },
    });

    expect(result.candidates).toHaveLength(1);
    expect(metadata.provider).toBe("ollama");
    expect(metadata.model).toBe("model-a");
    expect(metadata.contextWindow).toBe(8192);
    expect(metadata.durationMs).toBeGreaterThanOrEqual(0);
  });
});

describe("streamSemanticReview (streaming + cancellation)", () => {
  it("exposes the raw event stream to the caller", async () => {
    const provider = mockProvider({ id: "ollama" });
    const engine = createRuntimeEngine(createProviderRegistry([provider]));

    const {
      stream,
      provider: providerId,
      model,
    } = await engine.streamSemanticReview({
      units: [{ file: "a.ts", content: "x" }],
      selection: { mode: "automatic" },
    });

    const events: SemanticStreamEvent[] = [];
    for await (const event of stream) events.push(event);

    expect(providerId).toBe("ollama");
    expect(model).toBe("model-a");
    expect(events.some((e) => e.kind === "token")).toBe(true);
  });

  it("cancel() aborts the signal passed to the provider's execute", async () => {
    let receivedSignal: AbortSignal | undefined;
    const provider = mockProvider({
      id: "ollama",
      execute: async function* (request): AsyncIterable<SemanticStreamEvent> {
        receivedSignal = request.signal;
        yield { kind: "token", text: "partial" };
      },
    });
    const engine = createRuntimeEngine(createProviderRegistry([provider]));

    const { stream, cancel } = await engine.streamSemanticReview({
      units: [{ file: "a.ts", content: "x" }],
      selection: { mode: "automatic" },
    });

    // Drain one event to ensure execute() has started and captured the signal.
    const iterator = stream[Symbol.asyncIterator]();
    await iterator.next();

    cancel();
    expect(receivedSignal?.aborted).toBe(true);
  });

  it("bridges an externally-provided signal into the provider's request", async () => {
    let receivedSignal: AbortSignal | undefined;
    const provider = mockProvider({
      id: "ollama",
      execute: async function* (request): AsyncIterable<SemanticStreamEvent> {
        receivedSignal = request.signal;
        yield { kind: "token", text: "x" };
      },
    });
    const engine = createRuntimeEngine(createProviderRegistry([provider]));
    const externalController = new AbortController();

    const { stream } = await engine.streamSemanticReview({
      units: [{ file: "a.ts", content: "x" }],
      selection: { mode: "automatic" },
      signal: externalController.signal,
    });

    const iterator = stream[Symbol.asyncIterator]();
    await iterator.next();

    externalController.abort();
    expect(receivedSignal?.aborted).toBe(true);
  });
});
