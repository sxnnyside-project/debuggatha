import type { RunReviewOutput } from "./run-review.js";

/**
 * What every review tool returns. Findings carry `ledgerId`, the identity that
 * survives between reviews, so an agent can refer to the same finding in its next
 * call. `changes` is the delta it is really after: what this review found for the
 * first time, what it no longer finds, and what came back.
 *
 * A review that did not persist wrote nothing to the ledger, so a finding it saw
 * for the first time has no id to give.
 */
export function reviewPayload(
  output: RunReviewOutput,
  persist: boolean,
  extra: Record<string, unknown> = {},
): Record<string, unknown> {
  const { result, syncReport, suppressedFindings, suppressedInline, baselined } = output;
  const findings = result.findings.map((finding) => {
    const ledgerId = persist ? output.ledgerIds[finding.id] : undefined;
    return ledgerId ? { ...finding, ledgerId } : finding;
  });
  const withoutIds = <T extends { ledgerId: string }>(refs: T[]) =>
    refs.map(({ ledgerId: _dropped, ...rest }) => rest);

  return {
    summary: result.summary,
    findings,
    changes: {
      introduced: persist ? output.changes.introduced : withoutIds(output.changes.introduced),
      fixed: output.changes.fixed,
      reopened: output.changes.reopened,
    },
    syncReport,
    suppressedFindings,
    suppressedInline,
    baselined,
    analyzers: output.analyzers,
    semantic: output.semantic ?? null,
    persisted: persist,
    ...extra,
  };
}
