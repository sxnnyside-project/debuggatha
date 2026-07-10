import type { ReviewPack } from "@debuggatha/knowledge-system";

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
    },
    {
      id: "know-vue-v-if-v-for",
      title: "v-if with v-for",
      body: "In Vue 2, `v-for` had higher priority than `v-if`. In Vue 3, `v-if` has higher priority. Either way, mixing them on the same node causes unpredictable scope issues and performance loss. Pre-filtering lists via computed properties is significantly more efficient.",
      externalRefs: ["https://vuejs.org/style-guide/rules-essential.html#avoid-v-if-with-v-for"],
    },
    {
      id: "know-vue-composition",
      title: "Composition API Scalability",
      body: "The Options API organizes code by option type (data, methods, computed). For large components, this splits logical concerns across hundreds of lines. The Composition API allows grouping logic by feature, and provides vastly superior TypeScript type inference.",
      externalRefs: ["https://vuejs.org/guide/extras/composition-api-faq.html"],
    },
    {
      id: "know-vue-props",
      title: "One-Way Data Flow",
      body: "Props form a one-way down binding. Mutating a prop locally inside a child component causes anti-patterns where the parent's state falls out of sync with the child. Always use `emit('update:prop', newValue)`.",
      externalRefs: ["https://vuejs.org/guide/components/props.html#one-way-data-flow"],
    },
    {
      id: "know-vue-xss",
      title: "XSS via v-html",
      body: "The `v-html` directive renders raw HTML. Vue cannot sanitize this automatically. Rendering user-supplied content via `v-html` introduces severe Cross-Site Scripting vulnerabilities.",
      externalRefs: ["https://vuejs.org/guide/best-practices/security.html#html-injection"],
    },
  ],
};
