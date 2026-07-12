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
      limitations: [
        "A component that is presentational-looking but genuinely needs immediate interactivity above the fold (e.g. a hero-section search box or a cookie-consent banner that must respond to clicks the instant the page paints) legitimately warrants `client:load` — flagging every `client:load` usage without checking above-the-fold criticality produces false positives.",
      ],
    },
    {
      id: "know-astro-client-visible",
      title: "Lazy Hydration",
      body: "The `client:visible` directive uses an IntersectionObserver to load and hydrate JavaScript only when the component enters the viewport, vastly improving Time to Interactive (TTI) for initial pageloads.",
      externalRefs: ["https://docs.astro.build/en/reference/directives-reference/#clientvisible"],
      limitations: [
        "`client:visible` is unsuitable for a component whose interactivity must be ready before the user can plausibly reach it in a fast scroll (e.g. a sticky nav toggle that's visible on load) or one behind conditional rendering the IntersectionObserver can't reliably observe — the rule assumes the target is genuinely below-the-fold and scroll-triggered, which isn't true for every hydration candidate.",
      ],
    },
    {
      id: "know-astro-server-fetch",
      title: "Server-Side Fetching",
      body: "Code inside the Astro component frontmatter runs entirely on the server. Fetching data here allows Astro to render the HTML directly, meaning the client never has to make an API request or wait for a loading state.",
      externalRefs: ["https://docs.astro.build/en/guides/data-fetching/"],
      limitations: [
        "Data that is genuinely user-specific and must update without a full page reload (e.g. a live cart count or a client-side search-as-you-type against an API) cannot be fetched purely at frontmatter time — this rule applies to data known at build/request time, not to inherently client-driven interactive data needs.",
      ],
    },
    {
      id: "know-astro-scoped-css",
      title: "Scoped Styling",
      body: "Astro automatically scopes `<style>` tags to the component. Mixing global styles indiscriminately makes the codebase rigid and difficult to maintain.",
      externalRefs: ["https://docs.astro.build/en/guides/styling/#scoped-styles"],
      limitations: [
        "A root layout component intentionally defining site-wide typography, CSS resets, or design-token custom properties needs `is:global` by design — flagging any `is:global` usage without distinguishing a top-level layout file from a leaf component overstates the concern.",
      ],
    },
    {
      id: "know-astro-xss",
      title: "XSS via set:html",
      body: "Like `dangerouslySetInnerHTML`, `set:html` injects raw HTML directly into the document. Passing unsanitized content creates a critical XSS vulnerability.",
      externalRefs: ["https://docs.astro.build/en/reference/directives-reference/#sethtml"],
      limitations: [
        "`set:html` fed by content that passed through a trusted sanitizer (e.g. a markdown-to-HTML pipeline using `rehype-sanitize`, or CMS content already sanitized server-side) is safe — a rule that flags every `set:html` call regardless of whether its input is sanitized or truly unescaped user input will over-flag legitimate rendering of trusted rich content.",
      ],
    },
  ],
};
