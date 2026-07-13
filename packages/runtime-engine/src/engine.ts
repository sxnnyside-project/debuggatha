import { parseSemanticResponse } from "./parse-response.js";
import { buildSemanticPrompt } from "./prompt.js";
import type { Provider } from "./provider.js";
import type { ProviderRegistry } from "./registry.js";
import { consumeToCompletion } from "./stream.js";
import type {
  RuntimeExecutionMetadata,
  SemanticReviewResult,
  SemanticSourceUnit,
  SemanticStreamEvent,
} from "./types.js";

/**
 * Runtime Selection (Epic 13): the three modes the epic asks for.
 * `"automatic"` picks the first provider that reports healthy, then that
 * provider's first discovered model — a Review Skill never has to know
 * which one it got, only that it got *something* runnable, or a clear
 * error naming why not.
 */
export type RuntimeSelection =
  | { mode: "automatic" }
  | { mode: "provider"; providerId: string; model: string | undefined }
  | { mode: "model"; model: string };

export interface ResolvedRuntime {
  provider: Provider;
  model: string;
}

export class RuntimeSelectionError extends Error {}

async function firstHealthyProvider(providers: Provider[]): Promise<Provider> {
  for (const provider of providers) {
    const health = await provider.healthCheck();
    if (health.available) return provider;
  }
  throw new RuntimeSelectionError(
    `No registered provider is currently reachable (checked: ${providers.map((p) => p.id).join(", ") || "none registered"}).`,
  );
}

async function firstModelFor(provider: Provider): Promise<string> {
  const models = await provider.discoverModels();
  const first = models[0];
  if (!first) {
    throw new RuntimeSelectionError(
      `Provider "${provider.id}" is reachable but reports no available models.`,
    );
  }
  return first.id;
}

export async function resolveRuntime(
  registry: ProviderRegistry,
  selection: RuntimeSelection,
): Promise<ResolvedRuntime> {
  switch (selection.mode) {
    case "automatic": {
      const provider = await firstHealthyProvider(registry.list());
      return { provider, model: await firstModelFor(provider) };
    }
    case "provider": {
      const provider = registry.get(selection.providerId);
      if (!provider) {
        throw new RuntimeSelectionError(
          `No provider registered with id "${selection.providerId}" (registered: ${registry
            .list()
            .map((p) => p.id)
            .join(", ")}).`,
        );
      }
      return { provider, model: selection.model ?? (await firstModelFor(provider)) };
    }
    case "model": {
      for (const provider of registry.list()) {
        const models = await provider.discoverModels();
        if (models.some((m) => m.id === selection.model)) {
          return { provider, model: selection.model };
        }
      }
      throw new RuntimeSelectionError(
        `No registered provider currently serves model "${selection.model}".`,
      );
    }
  }
}

export interface RunSemanticReviewInput {
  units: SemanticSourceUnit[];
  selection: RuntimeSelection;
  policyStatements?: string[];
  /** Bridged into the internal controller the engine creates for this execution — aborting this signal cancels the provider request too. */
  signal?: AbortSignal;
}

export interface RunSemanticReviewOutput {
  result: SemanticReviewResult;
  metadata: RuntimeExecutionMetadata;
}

export interface StreamSemanticReviewOutput {
  stream: AsyncIterable<SemanticStreamEvent>;
  provider: string;
  model: string;
  /** Cancels this specific execution — propagates to the provider's own request (Epic 13 "Cancellation"). */
  cancel: () => void;
}

export interface RuntimeEngine {
  listProviders(): Provider[];
  resolveRuntime(selection: RuntimeSelection): Promise<ResolvedRuntime>;
  streamSemanticReview(input: RunSemanticReviewInput): Promise<StreamSemanticReviewOutput>;
  runSemanticReview(input: RunSemanticReviewInput): Promise<RunSemanticReviewOutput>;
}

/**
 * Bridges an external signal into an internal controller this execution
 * owns, so `cancel()` always has something to abort even when the caller
 * passed no signal at all.
 */
function ownedController(external: AbortSignal | undefined): AbortController {
  const controller = new AbortController();
  if (external) {
    if (external.aborted) controller.abort();
    else external.addEventListener("abort", () => controller.abort(), { once: true });
  }
  return controller;
}

export function createRuntimeEngine(registry: ProviderRegistry): RuntimeEngine {
  return {
    listProviders: () => registry.list(),
    resolveRuntime: (selection) => resolveRuntime(registry, selection),

    async streamSemanticReview(input) {
      const { provider, model } = await resolveRuntime(registry, input.selection);
      const contextWindow = await provider.contextWindowFor(model);
      const { prompt } = buildSemanticPrompt(
        input.units,
        { contextWindow, reservedTokens: 256 },
        input.policyStatements ?? [],
      );

      const controller = ownedController(input.signal);
      const stream = provider.execute({ model, prompt, signal: controller.signal });

      return {
        stream,
        provider: provider.id,
        model,
        cancel: () => controller.abort(),
      };
    },

    async runSemanticReview(input) {
      const { provider, model } = await resolveRuntime(registry, input.selection);
      const contextWindow = await provider.contextWindowFor(model);
      const { prompt, budget } = buildSemanticPrompt(
        input.units,
        { contextWindow, reservedTokens: 256 },
        input.policyStatements ?? [],
      );

      const controller = ownedController(input.signal);
      const startedAt = Date.now();
      const stream = provider.execute({ model, prompt, signal: controller.signal });
      const { result } = await consumeToCompletion(stream, parseSemanticResponse);
      const durationMs = Date.now() - startedAt;

      return {
        result,
        metadata: {
          provider: provider.id,
          model,
          contextWindow,
          durationMs,
          estimatedPromptTokens: budget.estimatedPromptTokens,
        },
      };
    },
  };
}
