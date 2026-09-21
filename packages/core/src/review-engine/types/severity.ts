/**
 * Severity and Confidence are deliberately separate types with no
 * conversion function between them — see the epic: "Never derive one
 * from the other." A Critical finding can be Low confidence (a
 * suspected but unverified SQL injection is still Critical if true) and
 * a Low finding can be High confidence (a definitely-unused import is
 * certain, but low-impact).
 *
 * Severity rubric — what happens if the finding is real and left alone:
 *   critical       an exploitable weakness or a leaked credential, with no other
 *                  condition needed (a live API key in source)
 *   high           a likely security weakness or a bug that ships wrong behavior
 *                  (`eval` on a value, a hardcoded password)
 *   medium         a defect or risk that shows up under some conditions
 *                  (loose equality, a non-null assertion, `any`)
 *   low            a maintainability or style cost, no behavior risk (`var`, string
 *                  concatenation)
 *   informational  worth knowing, nothing to fix
 * A style preference is never above `low`, however strict the linter that
 * ships it.
 *
 * Confidence rubric — how sure the detector is that the code is what the rule
 * describes, not that it is a bug:
 *   high    unambiguous syntax on real code (a call to `eval`, a `var` declaration)
 *   medium  a heuristic that is usually right (a high-entropy value assigned to `apiKey`)
 *   low     a weak signal that needs a person to judge (a short value assigned to `password`)
 */

export const SEVERITIES = ["critical", "high", "medium", "low", "informational"] as const;
export type Severity = (typeof SEVERITIES)[number];

export const CONFIDENCE_LEVELS = ["high", "medium", "low"] as const;
export type Confidence = (typeof CONFIDENCE_LEVELS)[number];
