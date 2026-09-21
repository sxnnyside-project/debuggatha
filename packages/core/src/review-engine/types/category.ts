/**
 * Categories stay extensible — a closed union would force a type change
 * (and a breaking change for every consumer) every time a new category
 * is needed. `Category` accepts any string; `CATEGORIES` names the
 * well-known ones for autocomplete and consistency.
 */
export const CATEGORIES = {
  Architecture: "architecture",
  Security: "security",
  Performance: "performance",
  Reliability: "reliability",
  Accessibility: "accessibility",
  Maintainability: "maintainability",
  DeveloperExperience: "developer-experience",
  Documentation: "documentation",
} as const;

export type KnownCategory = (typeof CATEGORIES)[keyof typeof CATEGORIES];
/** `string & {}` keeps editor autocomplete for KnownCategory while still accepting any string. */
export type Category = KnownCategory | (string & {});
