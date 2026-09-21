import type { Provider } from "../runtime-engine/provider.js";
import { createLMStudioProvider } from "../runtime-engine/providers/lmstudio.js";
import { createOllamaProvider } from "../runtime-engine/providers/ollama.js";
import type { CompletionRequest, SemanticProvider } from "./types.js";

const LOOPBACK = new Set(["localhost", "127.0.0.1", "[::1]", "::1"]);
const DEFAULT_TIMEOUT_MS = 120_000;

/**
 * Code is sent only to a model on this machine. A remote URL is refused: sending a
 * repository to a service is a decision for the person running it, and the host's own
 * model (sampling) is the way to use a hosted one.
 */
export function assertLoopback(url: string): URL {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error(`"${url}" is not a URL.`);
  }
  if (!LOOPBACK.has(parsed.hostname)) {
    throw new Error(
      `Refusing ${parsed.hostname}: the semantic layer sends code only to a model on this machine (localhost).`,
    );
  }
  return parsed;
}

/** A runtime's streaming provider as a question-and-answer one: deterministic, JSON where it can be enforced, bounded in time. */
export function fromRuntimeProvider(
  runtime: Provider,
  model: string,
  timeoutMs = DEFAULT_TIMEOUT_MS,
): SemanticProvider {
  return {
    name: runtime.id,
    model,
    async complete({ system, user, maxTokens }: CompletionRequest) {
      const signal = AbortSignal.timeout(timeoutMs);
      let text = "";
      for await (const event of runtime.execute({
        model,
        prompt: user,
        system,
        json: true,
        temperature: 0,
        maxTokens,
        signal,
      })) {
        if (event.kind === "token") text += event.text;
        else if (event.kind === "error") throw new Error(event.message);
      }
      // A provider treats an abort as the end of the stream, so the timeout has to be noticed here.
      if (signal.aborted)
        throw new Error(`${runtime.displayName} took longer than ${timeoutMs / 1000}s.`);
      if (!text.trim()) throw new Error(`${runtime.displayName} returned nothing.`);
      return text;
    },
  };
}

export interface LocalProviderOptions {
  baseUrl?: string;
  model?: string;
  timeoutMs?: number;
}

export type LocalProviderName = "ollama" | "lmstudio";
export const LOCAL_PROVIDERS: readonly LocalProviderName[] = ["ollama", "lmstudio"];

const DEFAULT_URL: Record<LocalProviderName, string> = {
  ollama: "http://localhost:11434",
  lmstudio: "http://localhost:1234",
};

/** Connects to a runtime on this machine and picks its model: the one asked for, or the first it has. */
export async function createLocalProvider(
  name: LocalProviderName,
  options: LocalProviderOptions = {},
): Promise<SemanticProvider> {
  const baseUrl = assertLoopback(options.baseUrl ?? DEFAULT_URL[name]).origin;
  const runtime =
    name === "ollama" ? createOllamaProvider({ baseUrl }) : createLMStudioProvider({ baseUrl });

  const health = await runtime.healthCheck();
  if (!health.available) throw new Error(`${runtime.displayName} is not answering at ${baseUrl}.`);

  let model = options.model;
  if (!model) {
    model = (await runtime.discoverModels())[0]?.id;
    if (!model) {
      throw new Error(
        name === "ollama"
          ? "Ollama has no models; pull one (`ollama pull qwen2.5-coder:7b`)."
          : "LM Studio has no model loaded.",
      );
    }
  }
  return fromRuntimeProvider(runtime, model, options.timeoutMs);
}

export const ollamaProvider = (options?: LocalProviderOptions) =>
  createLocalProvider("ollama", options);
export const lmStudioProvider = (options?: LocalProviderOptions) =>
  createLocalProvider("lmstudio", options);
