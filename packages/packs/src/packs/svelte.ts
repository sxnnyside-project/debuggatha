import type { ReviewPack } from "@debuggatha/core";

export const sveltePack: ReviewPack = {
  id: "debuggatha/svelte",
  version: "1.0.0",
  kind: "stack",
  displayName: "Svelte",
  dependsOn: [],
  rules: [
    {
      id: "reactive-assignments",
      packId: "debuggatha/svelte",
      statement:
        "Mutations to arrays or objects will not trigger Svelte reactivity unless an assignment operator (`=`) is used. E.g., `arr = [...arr, val]` instead of `arr.push(val)`.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "svelte" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-svelte-reactivity"],
      contradicts: undefined,
    },
    {
      id: "store-auto-subscription",
      packId: "debuggatha/svelte",
      statement:
        "Always use the `$` prefix (e.g., `$store`) to auto-subscribe and unsubscribe from Svelte stores inside components. Do not manually `store.subscribe` without handling `onDestroy`.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "svelte" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-svelte-stores"],
      contradicts: undefined,
    },
    {
      id: "no-complex-reactive-statements",
      packId: "debuggatha/svelte",
      statement:
        "Avoid putting complex logic or multiple side-effects inside a single `$: ` reactive statement to prevent unpredictable cascading updates.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "svelte" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-svelte-statements"],
      contradicts: undefined,
    },
    {
      id: "use-keyed-each",
      packId: "debuggatha/svelte",
      statement:
        "Always provide a unique key identifier when using `{#each items as item (item.id)}` to maintain state correctly across re-renders.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "svelte" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-svelte-keys"],
      contradicts: undefined,
    },
    {
      id: "unescaped-html",
      packId: "debuggatha/svelte",
      statement:
        "Avoid using `{@html content}`. If required, sanitize the `content` on the server or via a robust library like DOMPurify before rendering.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "svelte" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-svelte-xss"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-svelte-reactivity",
      title: "Assignment Reactivity",
      body: "Svelte's compiler uses assignment statements to determine when state has changed. Calling array methods like `.push()` mutates the array but does not notify the compiler to update the DOM. Reassignment (`arr = arr`) is required.",
      externalRefs: ["https://svelte.dev/tutorial/updating-arrays-and-objects"],
      limitations: [
        "In Svelte 5 with runes, a `$state([])` array is deeply reactive and `.push()`/`.splice()` mutations DO trigger updates correctly — this rule applies to Svelte 4 (and non-rune) reactivity semantics, so flagging `.push()` on a rune-based `$state` array in a Svelte 5 codebase is a false positive.",
      ],
    },
    {
      id: "know-svelte-stores",
      title: "Store Memory Leaks",
      body: "Subscribing to a store using `store.subscribe(val => ...)` returns an unsubscribe function. Failing to call this function in `onDestroy` creates a severe memory leak. The `$store` syntax handles this automatically.",
      externalRefs: ["https://svelte.dev/tutorial/auto-subscriptions"],
      limitations: [
        "A manual `store.subscribe()` call outside component scope (e.g. in a plain `.ts` module, a singleton service, or a one-time subscription intentionally kept alive for the app's lifetime) has no `onDestroy` lifecycle to hook into and is not a leak by omission — this rule applies specifically to subscriptions made inside a component instance.",
      ],
    },
    {
      id: "know-svelte-statements",
      title: "Cascading Reactive Statements",
      body: "Svelte topologically sorts `$: ` statements. Placing heavy operations or multiple side-effects into one block can cause unpredictable update loops and make the component logic incredibly hard to follow.",
      externalRefs: ["https://svelte.dev/tutorial/reactive-statements"],
      limitations: [
        "A single `$: ` block with multiple statements that are all simple, independent, and side-effect-free (e.g. deriving several unrelated display labels from the same prop) is not inherently a cascading-update risk — the concern is specifically statements with side effects or interdependent state, not block size or statement count alone.",
      ],
    },
    {
      id: "know-svelte-keys",
      title: "Keyed Each Blocks",
      body: "By default, Svelte modifies the DOM by adding/removing items at the end of a block and updating properties on existing nodes. This causes state leakage (like input focus) if a list is reordered. Keyed blocks tie state strictly to the item.",
      externalRefs: ["https://svelte.dev/tutorial/keyed-each-blocks"],
      limitations: [
        "An `{#each}` block over a list that is only ever appended to or fully replaced (never reordered, filtered, or spliced in place) has no state-leakage risk from unkeyed iteration — flagging every unkeyed `{#each}` regardless of whether the underlying list ever reorders produces false positives on static or append-only lists.",
      ],
    },
    {
      id: "know-svelte-xss",
      title: "XSS via @html",
      body: "The `{@html}` tag bypasses Svelte's built-in text escaping. Rendering unsanitized HTML is a direct vector for Cross-Site Scripting (XSS).",
      externalRefs: ["https://svelte.dev/tutorial/html-tags"],
      limitations: [
        "`{@html}` rendering a fully static, developer-authored string with no user- or externally-derived content interpolated into it (e.g. a hardcoded marketing snippet compiled at build time) carries no injection risk — the rule applies when the rendered content is, even partially, influenced by untrusted input.",
      ],
    },
  ],
};
