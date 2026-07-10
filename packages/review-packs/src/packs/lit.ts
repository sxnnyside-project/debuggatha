import type { ReviewPack } from "@debuggatha/knowledge-system";

export const litPack: ReviewPack = {
  id: "debuggatha/lit",
  version: "1.0.0",
  kind: "stack",
  displayName: "Lit",
  dependsOn: [],
  rules: [
    {
      id: "use-reactive-properties",
      packId: "debuggatha/lit",
      statement:
        "Define properties that affect rendering using the `@property()` decorator or static `properties` block to ensure the component re-renders when they change.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "lit" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-lit-reactivity"],
      contradicts: undefined,
    },
    {
      id: "avoid-manual-dom-manipulation",
      packId: "debuggatha/lit",
      statement:
        "Do not manually manipulate the DOM (e.g., `this.shadowRoot.appendChild`). Always express UI declaratively inside the `render()` method using `html` templates.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "lit" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-lit-rendering"],
      contradicts: undefined,
    },
    {
      id: "use-css-tagged-template",
      packId: "debuggatha/lit",
      statement:
        "Always define styles using the `css` tagged template literal rather than `<style>` blocks in the render method.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "lit" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-lit-styles"],
      contradicts: undefined,
    },
    {
      id: "event-binding-syntax",
      packId: "debuggatha/lit",
      statement:
        "Bind events using the `@event` syntax (e.g., `@click=${this.handleClick}`) instead of manually adding event listeners in lifecycle methods.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "lit" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-lit-events"],
      contradicts: undefined,
    },
    {
      id: "no-unsafe-html",
      packId: "debuggatha/lit",
      statement:
        "Avoid using the `unsafeHTML` directive. If necessary, heavily sanitize the input to prevent XSS.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "lit" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-lit-xss"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-lit-reactivity",
      title: "Lit Reactivity System",
      body: "Lit only re-renders when configured reactive properties change. Standard class properties will not trigger a render cycle, leading to a disconnected UI state.",
      externalRefs: ["https://lit.dev/docs/components/properties/"],
    },
    {
      id: "know-lit-rendering",
      title: "Declarative Rendering",
      body: "Lit's core value proposition is efficient DOM updates via tagged template literals. Imperative DOM manipulation circumvents this, destroying performance and introducing bugs.",
      externalRefs: ["https://lit.dev/docs/components/rendering/"],
    },
    {
      id: "know-lit-styles",
      title: "Constructable Stylesheets",
      body: "Using the `css` literal allows Lit to leverage Constructable Stylesheets. This ensures styles are parsed exactly once and shared across all instances of a component, drastically reducing memory overhead.",
      externalRefs: ["https://lit.dev/docs/components/styles/"],
    },
    {
      id: "know-lit-events",
      title: "Declarative Event Listeners",
      body: "Declarative event bindings (`@click`) are automatically added and removed by Lit. Manual `addEventListener` calls risk memory leaks if `removeEventListener` is forgotten during teardown.",
      externalRefs: [
        "https://lit.dev/docs/components/events/#adding-event-listeners-in-the-element-template",
      ],
    },
    {
      id: "know-lit-xss",
      title: "Unsafe HTML Injection",
      body: "Lit's `html` template automatically sanitizes bound values. Using the `unsafeHTML` directive disables this, opening the component to XSS.",
      externalRefs: ["https://lit.dev/docs/templates/directives/#unsafehtml"],
    },
  ],
};
