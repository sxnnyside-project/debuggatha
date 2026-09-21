import type { ReviewPack } from "@debuggatha/core";

export const vuePack: ReviewPack = {
  id: "debuggatha/vue",
  version: "1.0.0",
  kind: "stack",
  displayName: "Vue",
  dependsOn: [],
  rules: [
    {
      id: "v-for-key",
      packId: "debuggatha/vue",
      statement:
        "Always use `key` with `v-for`. The key must be a unique identifier, not the array index.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "vue" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-vue-keys"],
      contradicts: undefined,
    },
    {
      id: "no-v-if-with-v-for",
      packId: "debuggatha/vue",
      statement:
        "Never use `v-if` on the same element as `v-for`. Filter the list in a computed property instead.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "vue" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-vue-v-if-v-for"],
      contradicts: undefined,
    },
    {
      id: "prefer-composition-api",
      packId: "debuggatha/vue",
      statement:
        "Prefer using the Composition API (`<script setup>`) over the Options API for better TypeScript inference and logic reuse.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "vue" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-vue-composition"],
      contradicts: undefined,
    },
    {
      id: "mutating-props",
      packId: "debuggatha/vue",
      statement:
        "Do not mutate component props directly. Emit an event to the parent component to update the value.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "vue" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-vue-props"],
      contradicts: undefined,
    },
    {
      id: "use-v-html-carefully",
      packId: "debuggatha/vue",
      statement:
        "Avoid using `v-html`. If used, ensure the HTML string is strictly sanitized to prevent XSS attacks.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "vue" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-vue-xss"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-vue-keys",
      title: "Vue VDOM Reconciliation",
      body: "Vue uses an in-place patch strategy by default for lists. If the order of data items changes, Vue will not move the DOM elements, but will instead patch each element in place. Providing a unique `key` forces Vue to reorder elements properly.",
      externalRefs: ["https://vuejs.org/style-guide/rules-essential.html#use-keyed-v-for"],
      limitations: [
        "A list that is rendered once and never reordered, filtered, or spliced (e.g. a fixed set of static footer links) has no reconciliation correctness risk from a missing or index-based key — flagging every `v-for` without a stable key regardless of whether the underlying array ever mutates produces noise on genuinely static lists.",
      ],
    },
    {
      id: "know-vue-v-if-v-for",
      title: "v-if with v-for",
      body: "In Vue 2, `v-for` had higher priority than `v-if`. In Vue 3, `v-if` has higher priority. Either way, mixing them on the same node causes unpredictable scope issues and performance loss. Pre-filtering lists via computed properties is significantly more efficient.",
      externalRefs: ["https://vuejs.org/style-guide/rules-essential.html#avoid-v-if-with-v-for"],
      limitations: [
        "In Vue 3, a `v-if` on the same node as `v-for` that only ever checks a variable from outer scope (not the loop item itself, e.g. `v-if=\"isAdmin\"`) doesn't hit the per-item scope bug this rule warns about, since `v-if` now evaluates before the loop runs — it's still a style deviation from the recommended pattern, but not the correctness/performance hazard the rule describes for item-dependent conditions.",
      ],
    },
    {
      id: "know-vue-composition",
      title: "Composition API Scalability",
      body: "The Options API organizes code by option type (data, methods, computed). For large components, this splits logical concerns across hundreds of lines. The Composition API allows grouping logic by feature, and provides vastly superior TypeScript type inference.",
      externalRefs: ["https://vuejs.org/guide/extras/composition-api-faq.html"],
      limitations: [
        "A small, single-concern component (e.g. a simple presentational component with one or two props and no shared logic) gains little from Composition API's grouping benefit — recommending a rewrite for every Options API component regardless of size/complexity overstates the value for genuinely simple cases, and some teams intentionally standardize on Options API for onboarding-friendliness.",
      ],
    },
    {
      id: "know-vue-props",
      title: "One-Way Data Flow",
      body: "Props form a one-way down binding. Mutating a prop locally inside a child component causes anti-patterns where the parent's state falls out of sync with the child. Always use `emit('update:prop', newValue)`.",
      externalRefs: ["https://vuejs.org/guide/components/props.html#one-way-data-flow"],
      limitations: [
        "Mutating a nested property of an object/array prop for purely local, non-reactive-tracked bookkeeping (e.g. attaching a private `_internalCache` field to an object that the parent doesn't read back) doesn't create the parent-desync bug this rule targets — the risk is specifically mutating fields the parent's template or reactive state actually depends on.",
      ],
    },
    {
      id: "know-vue-xss",
      title: "XSS via v-html",
      body: "The `v-html` directive renders raw HTML. Vue cannot sanitize this automatically. Rendering user-supplied content via `v-html` introduces severe Cross-Site Scripting vulnerabilities.",
      externalRefs: ["https://vuejs.org/guide/best-practices/security.html#html-injection"],
      limitations: [
        "`v-html` bound exclusively to trusted, author-controlled content (e.g. HTML compiled at build time from local Markdown files with no user or third-party input in the pipeline) carries no XSS risk despite matching the pattern — the danger is about the data's trust boundary, not the directive itself.",
      ],
    },
  ],
};
