import type { Provider } from "../provider.js";
import type {
  HealthStatus,
  ModelInfo,
  SemanticExecutionRequest,
  SemanticStreamEvent,
} from "../types.js";
import { readNdjsonLines } from "./http-stream.js";

export interface OllamaProviderOptions {
  baseUrl?: string;
  /** Injectable for testing — defaults to the global `fetch`. */
  fetchImpl?: (input: string | URL, init?: RequestInit) => Promise<Response>;
}

interface OllamaTagsResponse {
  models?: { name: string }[];
}

interface OllamaGenerateChunk {
  response?: string;
  done?: boolean;
  error?: string;
}

/**
 * Production Ollama provider (Epic 13). Talks to Ollama's native HTTP
 * API (`/api/tags`, `/api/generate`, `/api/show`) — never leaks any of
 * this outside the module; callers only ever see the `Provider`
 * interface.
 */
export function createOllamaProvider(options: OllamaProviderOptions = {}): Provider {
  const baseUrl = options.baseUrl ?? "http://localhost:11434";
  const doFetch = options.fetchImpl ?? fetch;

  return {
    id: "ollama",
    displayName: "Ollama",

    async discoverModels(): Promise<ModelInfo[]> {
      try {
        const res = await doFetch(`${baseUrl}/api/tags`);
        if (!res.ok) return [];
        const body = (await res.json()) as OllamaTagsResponse;
        return (body.models ?? []).map((m) => ({
          id: m.name,
          provider: "ollama",
          contextWindow: undefined,
        }));
      } catch {
        return [];
      }
    },

    async healthCheck(): Promise<HealthStatus> {
      try {
        const res = await doFetch(`${baseUrl}/api/tags`);
        return res.ok
          ? { available: true, detail: `Reached Ollama at ${baseUrl}` }
          : { available: false, detail: `Ollama at ${baseUrl} responded with HTTP ${res.status}` };
      } catch (error) {
        return {
          available: false,
          detail: `Could not reach Ollama at ${baseUrl}: ${error instanceof Error ? error.message : String(error)}`,
        };
      }
    },

    async contextWindowFor(model: string): Promise<number | undefined> {
      try {
        const res = await doFetch(`${baseUrl}/api/show`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ name: model }),
        });
        if (!res.ok) return undefined;
        const body = (await res.json()) as { model_info?: Record<string, unknown> };
        const entry = Object.entries(body.model_info ?? {}).find(([key]) =>
          key.endsWith("context_length"),
        );
        return typeof entry?.[1] === "number" ? entry[1] : undefined;
      } catch {
        return undefined;
      }
    },

    async *execute(request: SemanticExecutionRequest): AsyncIterable<SemanticStreamEvent> {
      if (request.signal?.aborted) return;

      let res: Response;
      try {
        res = await doFetch(`${baseUrl}/api/generate`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ model: request.model, prompt: request.prompt, stream: true }),
          signal: request.signal ?? null,
        });
      } catch (error) {
        if ((error as { name?: string }).name === "AbortError") return;
        yield { kind: "error", message: error instanceof Error ? error.message : String(error) };
        return;
      }

      if (!res.ok || !res.body) {
        yield { kind: "error", message: `Ollama request failed with HTTP ${res.status}` };
        return;
      }

      try {
        for await (const rawChunk of readNdjsonLines(res)) {
          const chunk = rawChunk as OllamaGenerateChunk;
          if (chunk.error) {
            yield { kind: "error", message: chunk.error };
            return;
          }
          if (chunk.response) yield { kind: "token", text: chunk.response };
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
