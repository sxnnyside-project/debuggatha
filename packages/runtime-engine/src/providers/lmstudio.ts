import type { Provider } from "../provider.js";
import type {
  HealthStatus,
  ModelInfo,
  SemanticExecutionRequest,
  SemanticStreamEvent,
} from "../types.js";
import { readServerSentEvents } from "./http-stream.js";

export interface LMStudioProviderOptions {
  baseUrl?: string;
  /** Injectable for testing — defaults to the global `fetch`. */
  fetchImpl?: (input: string | URL, init?: RequestInit) => Promise<Response>;
}

interface LMStudioModelsResponse {
  data?: { id: string; max_context_length?: number }[];
}

interface OpenAiCompatibleChunk {
  choices?: { delta?: { content?: string }; finish_reason?: string | null }[];
  error?: { message?: string };
}

/**
 * Production LM Studio provider (Epic 13). LM Studio's local server
 * speaks the OpenAI chat-completions wire format
 * (`/v1/models`, `/v1/chat/completions`) — this module is the only place
 * that knows that; the rest of Debuggatha sees a `Provider`. Sharing the
 * same wire format is also why any future OpenAI-compatible endpoint
 * (Epic 13 "Examples: OpenAI-compatible endpoints") can likely reuse
 * `readServerSentEvents` and most of this file's shape.
 */
export function createLMStudioProvider(options: LMStudioProviderOptions = {}): Provider {
  const baseUrl = options.baseUrl ?? "http://localhost:1234";
  const doFetch = options.fetchImpl ?? fetch;

  const discoverModels = async (): Promise<ModelInfo[]> => {
    try {
      const res = await doFetch(`${baseUrl}/v1/models`);
      if (!res.ok) return [];
      const body = (await res.json()) as LMStudioModelsResponse;
      return (body.data ?? []).map((m) => ({
        id: m.id,
        provider: "lmstudio",
        contextWindow: typeof m.max_context_length === "number" ? m.max_context_length : undefined,
      }));
    } catch {
      return [];
    }
  };

  return {
    id: "lmstudio",
    displayName: "LM Studio",

    discoverModels,

    async healthCheck(): Promise<HealthStatus> {
      try {
        const res = await doFetch(`${baseUrl}/v1/models`);
        return res.ok
          ? { available: true, detail: `Reached LM Studio at ${baseUrl}` }
          : {
              available: false,
              detail: `LM Studio at ${baseUrl} responded with HTTP ${res.status}`,
            };
      } catch (error) {
        return {
          available: false,
          detail: `Could not reach LM Studio at ${baseUrl}: ${error instanceof Error ? error.message : String(error)}`,
        };
      }
    },

    async contextWindowFor(model: string): Promise<number | undefined> {
      const models = await discoverModels();
      return models.find((m) => m.id === model)?.contextWindow;
    },

    async *execute(request: SemanticExecutionRequest): AsyncIterable<SemanticStreamEvent> {
      if (request.signal?.aborted) return;

      let res: Response;
      try {
        res = await doFetch(`${baseUrl}/v1/chat/completions`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            model: request.model,
            messages: [{ role: "user", content: request.prompt }],
            stream: true,
          }),
          signal: request.signal ?? null,
        });
      } catch (error) {
        if ((error as { name?: string }).name === "AbortError") return;
        yield { kind: "error", message: error instanceof Error ? error.message : String(error) };
        return;
      }

      if (!res.ok || !res.body) {
        yield { kind: "error", message: `LM Studio request failed with HTTP ${res.status}` };
        return;
      }

      try {
        for await (const rawChunk of readServerSentEvents(res)) {
          const chunk = rawChunk as OpenAiCompatibleChunk;
          if (chunk.error?.message) {
            yield { kind: "error", message: chunk.error.message };
            return;
          }
          const text = chunk.choices?.[0]?.delta?.content;
          if (text) yield { kind: "token", text };
        }
      } catch (error) {
        if ((error as { name?: string }).name === "AbortError") return;
        yield { kind: "error", message: error instanceof Error ? error.message : String(error) };
        return;
      }

      yield { kind: "done" };
    },
  };
}
