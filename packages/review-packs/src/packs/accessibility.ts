import type { ReviewPack } from "@debuggatha/knowledge-system";

export const accessibilityPack: ReviewPack = {
  id: "debuggatha/accessibility",
  version: "1.0.0",
  kind: "concern",
  displayName: "Accessibility",
  dependsOn: [],
  rules: [
    {
      id: "img-alt-text",
      packId: "debuggatha/accessibility",
      statement:
        'All `<img>` elements must have an `alt` attribute. If the image is purely decorative, use `alt=""`.',
      category: "style",
      appliesTo: { kind: "always" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-a11y-alt-text"],
      contradicts: undefined,
    },
    {
      id: "aria-roles",
      packId: "debuggatha/accessibility",
      statement:
        'Do not use interactive ARIA roles (e.g., `role="button"`) on non-interactive elements (like `<div>` or `<span>`) without also implementing `tabindex` and keyboard event handlers (`keydown`, `keyup`). Prefer native semantic HTML.',
      category: "architecture",
      appliesTo: { kind: "always" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-a11y-roles"],
      contradicts: undefined,
    },
    {
      id: "form-labels",
      packId: "debuggatha/accessibility",
      statement:
        "Every form input (e.g., `<input>`, `<select>`, `<textarea>`) must have an associated `<label>`. Use the `for` attribute on the label pointing to the input's `id`, or nest the input inside the label.",
      category: "style",
      appliesTo: { kind: "always" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-a11y-labels"],
      contradicts: undefined,
    },
    {
      id: "focus-outline",
      packId: "debuggatha/accessibility",
      statement:
        "Do not set `outline: none` or `outline: 0` in CSS without providing a custom, high-contrast `:focus` state.",
      category: "style",
      appliesTo: { kind: "always" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-a11y-focus"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-a11y-alt-text",
      title: "Alternative Text for Images",
      body: "Screen readers rely entirely on the `alt` attribute to describe images to visually impaired users. Omitting the attribute causes the screen reader to read the image filename instead, providing a terrible user experience.",
      externalRefs: ["https://www.w3.org/WAI/tutorials/images/"],
    },
    {
      id: "know-a11y-roles",
      title: "Semantic HTML over ARIA",
      body: "Adding `role=\"button\"` to a `div` tells screen readers it is a button, but does not provide keyboard focus (`tabindex`) or trigger on Enter/Space key presses. Native `<button>` elements provide this for free. 'No ARIA is better than bad ARIA.'",
      externalRefs: ["https://www.w3.org/WAI/ARIA/apg/practices/read-me-first/"],
    },
    {
      id: "know-a11y-labels",
      title: "Accessible Forms",
      body: "Placeholders are not a substitute for labels. Screen readers cannot reliably associate placeholders with inputs. Labels ensure the input purpose is announced and they vastly increase the clickable hit area.",
      externalRefs: ["https://www.w3.org/WAI/tutorials/forms/labels/"],
    },
    {
      id: "know-a11y-focus",
      title: "Keyboard Focus Indicators",
      body: "Users who rely on keyboards must visually see which element has focus. Removing the default browser outline without providing a custom styling makes the application completely unusable for keyboard navigators.",
      externalRefs: ["https://www.w3.org/WAI/WCAG21/Understanding/focus-visible.html"],
    },
  ],
};
