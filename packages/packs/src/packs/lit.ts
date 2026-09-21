import type { ReviewPack } from "@debuggatha/core";

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
      limitations: [
        "A plain class field used only as an internal computation cache or a value read exclusively inside event handlers (never referenced from `render()`) has no need to be reactive — flagging every non-`@property`/`@state` class field as a bug ignores fields that intentionally don't affect the template output.",
      ],
    },
    {
      id: "know-lit-rendering",
      title: "Declarative Rendering",
      body: "Lit's core value proposition is efficient DOM updates via tagged template literals. Imperative DOM manipulation circumvents this, destroying performance and introducing bugs.",
      externalRefs: ["https://lit.dev/docs/components/rendering/"],
      limitations: [
        "Integrating a third-party imperative library that must directly own a DOM node (e.g. a charting library, a map widget, or a rich-text editor mounted into a ref'd container via `firstUpdated()`) requires direct DOM manipulation by design — this is Lit's own documented pattern for wrapping non-Lit widgets, not a violation of declarative rendering.",
      ],
    },
    {
      id: "know-lit-styles",
      title: "Constructable Stylesheets",
      body: "Using the `css` literal allows Lit to leverage Constructable Stylesheets. This ensures styles are parsed exactly once and shared across all instances of a component, drastically reducing memory overhead.",
      externalRefs: ["https://lit.dev/docs/components/styles/"],
      limitations: [
        "A `<style>` block injected inside `render()` specifically to apply per-instance dynamic values that `css`'s static tagged-template can't parameterize (before CSS custom properties were adopted for that value) is a legitimate, if less common, escape hatch — flagging every in-template `<style>` block identically to hardcoded static styles misses this dynamic-styling case.",
      ],
    },
    {
      id: "know-lit-events",
      title: "Declarative Event Listeners",
      body: "Declarative event bindings (`@click`) are automatically added and removed by Lit. Manual `addEventListener` calls risk memory leaks if `removeEventListener` is forgotten during teardown.",
      externalRefs: [
        "https://lit.dev/docs/components/events/#adding-event-listeners-in-the-element-template",
      ],
      limitations: [
        "Listening on `window`, `document`, or another element outside the component's own template (e.g. a global resize or scroll listener) has no `@event` template syntax equivalent and must be added imperatively, typically in `connectedCallback`/`disconnectedCallback` — flagging manual `addEventListener` usage uniformly, without distinguishing listeners on the component's own rendered nodes from listeners on external targets, misses this legitimate necessity.",
      ],
    },
    {
      id: "know-lit-xss",
      title: "Unsafe HTML Injection",
      body: "Lit's `html` template automatically sanitizes bound values. Using the `unsafeHTML` directive disables this, opening the component to XSS.",
      externalRefs: ["https://lit.dev/docs/templates/directives/#unsafehtml"],
      limitations: [
        "`unsafeHTML` applied to content that has already passed through a dedicated sanitization library (e.g. DOMPurify) or to fully static, developer-authored markup with zero user input in its construction is the directive's documented safe usage — flagging every `unsafeHTML` call site identically, regardless of whether the bound value was actually sanitized, produces false positives on correctly-guarded rendering.",
      ],
    },
  ],
};
