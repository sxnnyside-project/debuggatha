import { describe, expect, it, vi } from "vitest";
import { createOllamaProvider } from "./ollama.js";

function ndjsonResponse(lines: unknown[]): Response {
  const body = new ReadableStream({
    start(controller) {
      const encoder = new TextEncoder();
      for (const line of lines) controller.enqueue(encoder.encode(`${JSON.stringify(line)}\n`));
      controller.close();
    },
  });
  return new Response(body, { status: 200 });
}

describe("createOllamaProvider", () => {
  it("discovers models from /api/tags", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ models: [{ name: "llama3" }, { name: "codellama" }] }), {
        status: 200,
      }),
    );
    const provider = createOllamaProvider({ baseUrl: "http://x", fetchImpl });

    const models = await provider.discoverModels();
    expect(models.map((m) => m.id)).toEqual(["llama3", "codellama"]);
    expect(fetchImpl).toHaveBeenCalledWith("http://x/api/tags");
  });

  it("returns an empty model list instead of throwing when unreachable", async () => {
    const fetchImpl = vi.fn().mockRejectedValue(new Error("ECONNREFUSED"));
    const provider = createOllamaProvider({ baseUrl: "http://x", fetchImpl });
    expect(await provider.discoverModels()).toEqual([]);
  });

  it("reports healthy when /api/tags responds ok", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(new Response("{}", { status: 200 }));
    const provider = createOllamaProvider({ baseUrl: "http://x", fetchImpl });
    expect((await provider.healthCheck()).available).toBe(true);
  });

  it("reports unhealthy with a clear detail when unreachable", async () => {
    const fetchImpl = vi.fn().mockRejectedValue(new Error("ECONNREFUSED"));
    const provider = createOllamaProvider({ baseUrl: "http://x", fetchImpl });
    const health = await provider.healthCheck();
    expect(health.available).toBe(false);
    expect(health.detail).toMatch(/ECONNREFUSED/);
  });

  it("streams token events parsed from NDJSON lines", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(
        ndjsonResponse([
          { response: "Hello ", done: false },
          { response: "world", done: false },
          { done: true },
        ]),
      );
    const provider = createOllamaProvider({ baseUrl: "http://x", fetchImpl });

    const events = [];
    for await (const event of provider.execute({
      model: "llama3",
      prompt: "hi",
      signal: undefined,
    })) {
      events.push(event);
    }

    expect(events).toEqual([
      { kind: "token", text: "Hello " },
      { kind: "token", text: "world" },
      { kind: "done" },
    ]);
  });

  it("yields an error event instead of throwing on a non-ok response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(new Response("nope", { status: 500 }));
    const provider = createOllamaProvider({ baseUrl: "http://x", fetchImpl });

    const events = [];
    for await (const event of provider.execute({
      model: "llama3",
      prompt: "hi",
      signal: undefined,
    })) {
      events.push(event);
    }
    expect(events).toEqual([{ kind: "error", message: "Ollama request failed with HTTP 500" }]);
  });

  it("yields nothing for an already-aborted signal", async () => {
    const fetchImpl = vi.fn();
    const provider = createOllamaProvider({ baseUrl: "http://x", fetchImpl });
    const controller = new AbortController();
    controller.abort();

    const events = [];
    for await (const event of provider.execute({
      model: "llama3",
      prompt: "hi",
      signal: controller.signal,
    })) {
      events.push(event);
    }
    expect(events).toEqual([]);
    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it("reads context window from /api/show's model_info", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ model_info: { "llama.context_length": 8192 } }), {
        status: 200,
      }),
    );
    const provider = createOllamaProvider({ baseUrl: "http://x", fetchImpl });
    expect(await provider.contextWindowFor("llama3")).toBe(8192);
  });
});
