import type { ReviewPack } from "@debuggatha/knowledge-system";

export const astroPack: ReviewPack = {
  id: "debuggatha/astro",
  version: "1.0.0",
  kind: "stack",
  displayName: "Astro",
  dependsOn: [],
  rules: [
    {
      id: "islands-architecture",
      packId: "debuggatha/astro",
      statement:
        "Do not hydrate UI components unless absolutely necessary for interactivity. Avoid using `client:load` on components that are purely presentational.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "astro" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-astro-islands"],
      contradicts: undefined,
    },
    {
      id: "prefer-client-visible",
      packId: "debuggatha/astro",
      statement:
        "When hydrating components below the fold, use `client:visible` instead of `client:load` to defer JavaScript execution until the component enters the viewport.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "astro" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-astro-client-visible"],
      contradicts: undefined,
    },
    {
      id: "server-side-fetch",
      packId: "debuggatha/astro",
      statement:
        "Perform heavy data fetching inside the frontmatter (the `---` block) of the `.astro` component to ensure it runs exclusively on the server at build or request time.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "astro" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-astro-server-fetch"],
      contradicts: undefined,
    },
    {
      id: "avoid-global-styles",
      packId: "debuggatha/astro",
      statement:
        "Scope CSS within `.astro` components by default. Use `is:global` sparingly to prevent style bleed across pages.",
      category: "style",
      appliesTo: { kind: "requires-framework", framework: "astro" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-astro-scoped-css"],
      contradicts: undefined,
    },
    {
      id: "set-html-security",
      packId: "debuggatha/astro",
      statement:
        "Avoid using `set:html` with unescaped user input, as it exposes the site to Cross-Site Scripting (XSS).",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "astro" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-astro-xss"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-astro-islands",
      title: "Zero JS by Default",
      body: "Astro ships zero JavaScript to the client by default. Hydrating components needlessly with `client:load` destroys the primary performance benefit of the framework. Reserve hydration strictly for interactive 'islands'.",
      externalRefs: ["https://docs.astro.build/en/concepts/islands/"],
    },
    {
      id: "know-astro-client-visible",
      title: "Lazy Hydration",
      body: "The `client:visible` directive uses an IntersectionObserver to load and hydrate JavaScript only when the component enters the viewport, vastly improving Time to Interactive (TTI) for initial pageloads.",
      externalRefs: ["https://docs.astro.build/en/reference/directives-reference/#clientvisible"],
    },
    {
      id: "know-astro-server-fetch",
      title: "Server-Side Fetching",
      body: "Code inside the Astro component frontmatter runs entirely on the server. Fetching data here allows Astro to render the HTML directly, meaning the client never has to make an API request or wait for a loading state.",
      externalRefs: ["https://docs.astro.build/en/guides/data-fetching/"],
    },
    {
      id: "know-astro-scoped-css",
      title: "Scoped Styling",
      body: "Astro automatically scopes `<style>` tags to the component. Mixing global styles indiscriminately makes the codebase rigid and difficult to maintain.",
      externalRefs: ["https://docs.astro.build/en/guides/styling/#scoped-styles"],
    },
    {
      id: "know-astro-xss",
      title: "XSS via set:html",
      body: "Like `dangerouslySetInnerHTML`, `set:html` injects raw HTML directly into the document. Passing unsanitized content creates a critical XSS vulnerability.",
      externalRefs: ["https://docs.astro.build/en/reference/directives-reference/#sethtml"],
    },
  ],
};
