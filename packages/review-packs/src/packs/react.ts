import type { ReviewPack } from "@debuggatha/knowledge-system";

export const reactPack: ReviewPack = {
  id: "debuggatha/react",
  version: "1.0.0",
  kind: "stack",
  displayName: "React",
  dependsOn: [],
  rules: [
    {
      id: "rules-of-hooks",
      packId: "debuggatha/react",
      statement:
        "Never call Hooks inside loops, conditions, or nested functions. Always use Hooks at the top level of a React functional component.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "react" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-react-hooks"],
      contradicts: undefined,
    },
    {
      id: "exhaustive-deps",
      packId: "debuggatha/react",
      statement:
        "Always include all variables from the component scope used inside `useEffect`, `useCallback`, or `useMemo` in the dependency array.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "react" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-react-deps"],
      contradicts: undefined,
    },
    {
      id: "no-array-index-key",
      packId: "debuggatha/react",
      statement:
        "Avoid using the array index as the `key` prop in mapped elements unless the list is completely static and never reordered.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "react" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-react-keys"],
      contradicts: undefined,
    },
    {
      id: "no-dangerously-set-inner-html",
      packId: "debuggatha/react",
      statement:
        "Avoid `dangerouslySetInnerHTML`. If necessary, ensure the input string is heavily sanitized (e.g., using DOMPurify) to prevent XSS.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "react" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-react-xss"],
      contradicts: undefined,
    },
    {
      id: "state-colocation",
      packId: "debuggatha/react",
      statement:
        "Colocate state as close to where it is used as possible. Avoid pushing transient UI state to global stores (e.g., Redux, Context).",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "react" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-react-colocation"],
      contradicts: undefined,
    },
    {
      id: "memoization-overhead",
      packId: "debuggatha/react",
      statement:
        "Do not blindly wrap every component in `React.memo` or every function in `useCallback`. Memoization carries an inherent comparison overhead.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "react" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-react-memo"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-react-hooks",
      title: "Rules of Hooks",
      body: "React relies on the call order of Hooks to match state and effects with the correct component instance. Conditional or looped hook calls scramble this order, causing catastrophic runtime bugs and state corruption.",
      externalRefs: ["https://legacy.reactjs.org/docs/hooks-rules.html"],
    },
    {
      id: "know-react-deps",
      title: "Exhaustive Dependencies",
      body: "Omitting a dependency from `useEffect` means the effect will 'see' a stale closure of that variable from a previous render. This leads to extremely difficult-to-debug logic errors.",
      externalRefs: [
        "https://github.com/facebook/react/tree/main/packages/eslint-plugin-react-hooks",
      ],
    },
    {
      id: "know-react-keys",
      title: "Reconciliation and Keys",
      body: "React uses the `key` prop to identify which items have changed, been added, or been removed. Using the array index as a key forces React to re-render all elements if the list is sorted or an item is deleted at the top.",
      externalRefs: ["https://react.dev/learn/rendering-lists#why-does-react-need-keys"],
    },
    {
      id: "know-react-xss",
      title: "Cross-Site Scripting via DOM Injection",
      body: "React is generally safe against XSS because it escapes strings in JSX. `dangerouslySetInnerHTML` bypasses this protection entirely. Injecting unsanitized user content here leads to arbitrary JavaScript execution.",
      externalRefs: [
        "https://react.dev/reference/react-dom/components/common#dangerouslysetinnerhtml",
      ],
    },
    {
      id: "know-react-colocation",
      title: "State Colocation",
      body: "Global state triggers renders across vast swathes of the component tree. Pushing local UI state (like 'is a dropdown open') into a global store causes unnecessary cascading re-renders and couples isolated components.",
      externalRefs: [
        "https://kentcdodds.com/blog/state-colocation-will-make-your-react-app-faster",
      ],
    },
    {
      id: "know-react-memo",
      title: "Premature Memoization",
      body: "React is highly optimized to re-render fast. `React.memo` requires a shallow equality check on props. If props change frequently, or if the component is very cheap to render, the memoization check itself becomes a performance bottleneck.",
      externalRefs: ["https://kentcdodds.com/blog/usememo-and-usecallback"],
    },
  ],
};
