import { deepFreeze } from "./internal/deep-freeze.js";
import type { MemoryItem } from "./types/item.js";
import { transitionMemory } from "./types/item.js";
import type { MemoryStore } from "./types/store.js";

function withItems(store: MemoryStore, items: MemoryItem[]): MemoryStore {
  return deepFreeze({ ...store, items, updatedAt: new Date().toISOString() });
}

function replaceItem(store: MemoryStore, updated: MemoryItem): MemoryStore {
  return withItems(
    store,
    store.items.map((item) => (item.id === updated.id ? updated : item)),
  );
}

function getItemOrThrow(store: MemoryStore, itemId: string): MemoryItem {
  const item = store.items.find((i) => i.id === itemId);
  if (!item) throw new Error(`No memory item with id "${itemId}" in this store.`);
  return item;
}

/** Appends a newly created item (see `createMemoryItem`, `types/item.js`) to the store. */
export function addMemoryItem(store: MemoryStore, item: MemoryItem): MemoryStore {
  return withItems(store, [...store.items, item]);
}

/**
 * `detected` -> `suggested`: surfaces an item for user review. The one
 * transition a detection pass drives on its own, deliberately —
 * everything past this point requires a real user action.
 */
export function suggestMemory(store: MemoryStore, itemId: string, sessionId: string): MemoryStore {
  const item = getItemOrThrow(store, itemId);
  return replaceItem(store, transitionMemory(item, "suggested", { kind: "detection", sessionId }));
}

/**
 * User Experience: "confirm memory." The *only* path that can produce
 * `confidence: "user-confirmed"` semantics — this function doesn't
 * change `confidence` itself (that's set at creation), but it's the only
 * lifecycle transition gated on an explicit user actor, never a
 * `"detection"` origin, keeping "never present inferred knowledge as
 * confirmed fact" true at the API boundary, not just by convention.
 */
export function confirmMemory(
  store: MemoryStore,
  itemId: string,
  actor: string,
  comment?: string,
): MemoryStore {
  const item = getItemOrThrow(store, itemId);
  return replaceItem(store, transitionMemory(item, "confirmed", { kind: "user", actor }, comment));
}

/** `confirmed` -> `active`: starts actually influencing reviews (see `filter.ts`). */
export function activateMemory(store: MemoryStore, itemId: string, actor: string): MemoryStore {
  const item = getItemOrThrow(store, itemId);
  return replaceItem(store, transitionMemory(item, "active", { kind: "user", actor }));
}

/** User Experience: "reject memory" — a `detected`/`suggested` item a user declines goes straight to `archived`; it was never true, so there's nothing to deprecate. */
export function rejectMemory(
  store: MemoryStore,
  itemId: string,
  actor: string,
  comment?: string,
): MemoryStore {
  const item = getItemOrThrow(store, itemId);
  return replaceItem(store, transitionMemory(item, "archived", { kind: "user", actor }, comment));
}

/** `active` -> `deprecated`: stops influencing new reviews but stays inspectable, e.g. when the repository changed enough that the item might no longer hold. */
export function deprecateMemory(
  store: MemoryStore,
  itemId: string,
  actor: string,
  comment?: string,
): MemoryStore {
  const item = getItemOrThrow(store, itemId);
  return replaceItem(store, transitionMemory(item, "deprecated", { kind: "user", actor }, comment));
}

/** `deprecated` -> `active`: a deprecated item that turns out to still hold can be reactivated instead of being recreated from scratch. */
export function reactivateMemory(store: MemoryStore, itemId: string, actor: string): MemoryStore {
  const item = getItemOrThrow(store, itemId);
  return replaceItem(store, transitionMemory(item, "active", { kind: "user", actor }));
}

/** Terminal archive from any non-archived state — kept for audit, never consulted again (see `isActiveMemoryStatus`). */
export function archiveMemory(
  store: MemoryStore,
  itemId: string,
  actor: string,
  comment?: string,
): MemoryStore {
  const item = getItemOrThrow(store, itemId);
  return replaceItem(store, transitionMemory(item, "archived", { kind: "user", actor }, comment));
}

/**
 * User Experience: "remove memory" — distinct from archiving. Archiving
 * keeps a permanent, inspectable record; removing deletes the item from
 * the store entirely. Both are offered because "I don't want this
 * remembered anymore" (remove) and "this was true but no longer applies,
 * keep the history" (archive) are different user intents.
 */
export function removeMemoryItem(store: MemoryStore, itemId: string): MemoryStore {
  getItemOrThrow(store, itemId); // throws if missing, for the same clear-failure reason every other api.ts function does
  return withItems(
    store,
    store.items.filter((item) => item.id !== itemId),
  );
}
