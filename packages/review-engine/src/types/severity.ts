/**
 * Severity and Confidence are deliberately separate types with no
 * conversion function between them — see the epic: "Never derive one
 * from the other." A Critical finding can be Low confidence (a
 * suspected but unverified SQL injection is still Critical if true) and
 * a Low finding can be High confidence (a definitely-unused import is
 * certain, but low-impact).
 */

export const SEVERITIES = ["critical", "high", "medium", "low", "informational"] as const;
export type Severity = (typeof SEVERITIES)[number];

export const CONFIDENCE_LEVELS = ["high", "medium", "low"] as const;
export type Confidence = (typeof CONFIDENCE_LEVELS)[number];
