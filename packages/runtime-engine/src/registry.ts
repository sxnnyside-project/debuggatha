import type { Provider } from "./provider.js";

/**
 * Injectable, in-memory — same discipline as every other epic's registry
 * (`CapabilityRegistry`, `RepositoryContextCache`, `AnalysisCache`): no
 * hidden global singleton. Adding a new provider (an OpenAI-compatible
 * endpoint, an Anthropic-compatible one, a future MCP runtime) means
 * writing a new `Provider` implementation and registering it here —
 * nothing in `engine.ts` or any Review Skill changes (Epic 13 "Design
 * the architecture so additional providers can be added without
 * modifying existing code").
 */
export interface ProviderRegistry {
  register(provider: Provider): void;
  get(id: string): Provider | undefined;
  list(): Provider[];
}

export function createProviderRegistry(initial: Provider[] = []): ProviderRegistry {
  const byId = new Map<string, Provider>();
  for (const provider of initial) byId.set(provider.id, provider);

  return {
    register(provider) {
      byId.set(provider.id, provider);
    },
    get(id) {
      return byId.get(id);
    },
    list() {
      return [...byId.values()];
    },
  };
}
