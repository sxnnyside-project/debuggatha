import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { computeFindingFingerprint, type Finding } from "@debuggatha/core";

/**
 * Adoption mode. A repository that has never been reviewed shows hundreds of
 * findings on day one, and nobody starts fixing hundreds. A baseline records
 * what is already there so reviews report only what is new; the ledger still
 * tracks everything, so nothing is forgotten, only quieted.
 */

export const BASELINE_FILE = join(".debuggatha", "baseline.json");
const SCHEMA_VERSION = 1;

export interface BaselineEntry {
  key: string;
  file: string;
  ruleId: string;
  title: string;
  /** How many identical findings (same file, rule, and code) were accepted. */
  count: number;
}

export interface Baseline {
  schemaVersion: number;
  createdAt: string;
  entries: BaselineEntry[];
}

/** A finding's identity for baselining: where it is, which rule, and the code it is about; never a line number, so unrelated edits do not resurface it. */
export function baselineKey(finding: Finding): string {
  const fingerprint = computeFindingFingerprint(finding);
  return [fingerprint.file, fingerprint.ruleId, fingerprint.contentAnchor ?? ""].join("::");
}

export function baselinePath(rootDir: string): string {
  return join(rootDir, BASELINE_FILE);
}

export function loadBaseline(rootDir: string): Baseline | undefined {
  const path = baselinePath(rootDir);
  if (!existsSync(path)) return undefined;
  let parsed: unknown;
  try {
    parsed = JSON.parse(readFileSync(path, "utf8"));
  } catch {
    throw new Error(
      `${BASELINE_FILE} is not valid JSON. Delete it or recreate it with a new baseline.`,
    );
  }
  const baseline = parsed as Partial<Baseline>;
  if (baseline.schemaVersion !== SCHEMA_VERSION || !Array.isArray(baseline.entries)) {
    throw new Error(
      `${BASELINE_FILE} has an unsupported format (schemaVersion ${String(baseline.schemaVersion)}). Recreate it with a new baseline.`,
    );
  }
  return baseline as Baseline;
}

export function createBaselineFrom(findings: readonly Finding[]): Baseline {
  const entries = new Map<string, BaselineEntry>();
  for (const finding of findings) {
    const key = baselineKey(finding);
    const existing = entries.get(key);
    if (existing) {
      existing.count += 1;
      continue;
    }
    const fingerprint = computeFindingFingerprint(finding);
    entries.set(key, {
      key,
      file: fingerprint.file,
      ruleId: fingerprint.ruleId ?? "",
      title: finding.title,
      count: 1,
    });
  }
  return {
    schemaVersion: SCHEMA_VERSION,
    createdAt: new Date().toISOString(),
    entries: [...entries.values()].sort((a, b) => a.key.localeCompare(b.key)),
  };
}

export function saveBaseline(rootDir: string, baseline: Baseline): void {
  const path = baselinePath(rootDir);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(baseline, null, 2)}\n`);
}

export function clearBaseline(rootDir: string): boolean {
  const path = baselinePath(rootDir);
  if (!existsSync(path)) return false;
  rmSync(path);
  return true;
}

/** Splits findings into those the baseline already accepted and those that are new; identical findings are matched one for one. */
export function applyBaseline(
  findings: readonly Finding[],
  baseline: Baseline,
): { fresh: Finding[]; baselined: Finding[] } {
  const remaining = new Map(baseline.entries.map((entry) => [entry.key, entry.count]));
  const fresh: Finding[] = [];
  const baselined: Finding[] = [];
  for (const finding of findings) {
    const key = baselineKey(finding);
    const left = remaining.get(key) ?? 0;
    if (left > 0) {
      remaining.set(key, left - 1);
      baselined.push(finding);
    } else {
      fresh.push(finding);
    }
  }
  return { fresh, baselined };
}
