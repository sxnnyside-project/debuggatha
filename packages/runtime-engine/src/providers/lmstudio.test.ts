import { describe, expect, it, vi } from "vitest";
import { createLMStudioProvider } from "./lmstudio.js";

function sseResponse(chunks: unknown[]): Response {
  const body = new ReadableStream({
    start(controller) {
      const encoder = new TextEncoder();
      for (const chunk of chunks)
        controller.enqueue(encoder.encode(`data: ${JSON.stringify(chunk)}\n\n`));
      controller.enqueue(encoder.encode("data: [DONE]\n\n"));
      controller.close();
    },
  });
  return new Response(body, { status: 200 });
}

describe("createLMStudioProvider", () => {
  it("discovers models from /v1/models, including context length when reported", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({ data: [{ id: "qwen", max_context_length: 32768 }, { id: "phi" }] }),
        {
          status: 200,
        },
      ),
    );
    const provider = createLMStudioProvider({ baseUrl: "http://x", fetchImpl });

    const models = await provider.discoverModels();
    expect(models).toEqual([
      { id: "qwen", provider: "lmstudio", contextWindow: 32768 },
      { id: "phi", provider: "lmstudio", contextWindow: undefined },
    ]);
  });

  it("reports unhealthy with a clear detail on a non-ok response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(new Response("", { status: 503 }));
    const provider = createLMStudioProvider({ baseUrl: "http://x", fetchImpl });
    const health = await provider.healthCheck();
    expect(health.available).toBe(false);
    expect(health.detail).toMatch(/503/);
  });

  it("streams token events parsed from OpenAI-compatible SSE chunks", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(
        sseResponse([
          { choices: [{ delta: { content: "Hel" } }] },
          { choices: [{ delta: { content: "lo" } }] },
        ]),
      );
    const provider = createLMStudioProvider({ baseUrl: "http://x", fetchImpl });

    const events = [];
    for await (const event of provider.execute({
      model: "qwen",
      prompt: "hi",
      signal: undefined,
    })) {
      events.push(event);
    }
    expect(events).toEqual([
      { kind: "token", text: "Hel" },
      { kind: "token", text: "lo" },
      { kind: "done" },
    ]);
  });

  it("surfaces a provider-reported error mid-stream instead of silently stopping", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(sseResponse([{ error: { message: "model unloaded" } }]));
    const provider = createLMStudioProvider({ baseUrl: "http://x", fetchImpl });

    const events = [];
    for await (const event of provider.execute({
      model: "qwen",
      prompt: "hi",
      signal: undefined,
    })) {
      events.push(event);
    }
    expect(events).toEqual([{ kind: "error", message: "model unloaded" }]);
  });

  it("resolves context window for a specific model via discoverModels", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ data: [{ id: "qwen", max_context_length: 32768 }] }), {
        status: 200,
      }),
    );
    const provider = createLMStudioProvider({ baseUrl: "http://x", fetchImpl });
    expect(await provider.contextWindowFor("qwen")).toBe(32768);
    expect(await provider.contextWindowFor("unknown-model")).toBeUndefined();
  });
});
