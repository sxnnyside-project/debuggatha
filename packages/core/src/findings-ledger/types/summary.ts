import type { Severity } from "../../review-engine/index.js";
import { SEVERITIES } from "../../review-engine/index.js";
import type { Ledger } from "./ledger.js";
import type { FindingLifecycleStatus } from "./lifecycle.js";

/**
 * Repository-level summary (epic §7) — reusable by any future adapter
 * (CLI, MCP, VS Code, CI). `bySeverity`/`byCategory` count only the
 * *active* statuses (open, acknowledged, reopened) — the actionable set —
 * while `countsByStatus` gives the full lifecycle picture including
 * resolved/dismissed history.
 */
export interface LedgerSummary {
  countsByStatus: Record<FindingLifecycleStatus, number>;
  activeBySeverity: Record<Severity, number>;
  activeByCategory: Record<string, number>;
}

export function summarizeLedger(ledger: Ledger): LedgerSummary {
  const countsByStatus = {
    open: 0,
    acknowledged: 0,
    resolved: 0,
    dismissed: 0,
    reopened: 0,
  } as Record<FindingLifecycleStatus, number>;
  const activeBySeverity = Object.fromEntries(SEVERITIES.map((s) => [s, 0])) as Record<
    Severity,
    number
  >;
  const activeByCategory: Record<string, number> = {};

  for (const entry of ledger.entries) {
    countsByStatus[entry.status] += 1;

    if (entry.status === "open" || entry.status === "acknowledged" || entry.status === "reopened") {
      activeBySeverity[entry.latestFinding.severity] += 1;
      activeByCategory[entry.latestFinding.category] =
        (activeByCategory[entry.latestFinding.category] ?? 0) + 1;
    }
  }

  return { countsByStatus, activeBySeverity, activeByCategory };
}
