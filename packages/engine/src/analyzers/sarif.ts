import type { Severity } from "@debuggatha/core";
import { toRepositoryPath } from "./paths.js";
import type { AnalyzerFinding, ParseContext } from "./types.js";

interface SarifRegion {
  startLine?: number;
  startColumn?: number;
  endLine?: number;
  endColumn?: number;
}

interface SarifResult {
  ruleId?: string;
  level?: string;
  message?: { text?: string };
  locations?: {
    physicalLocation?: { artifactLocation?: { uri?: string }; region?: SarifRegion };
  }[];
}

interface SarifRule {
  id?: string;
  helpUri?: string;
  properties?: Record<string, unknown>;
}

interface SarifRun {
  tool?: { driver?: { rules?: SarifRule[]; version?: string; semanticVersion?: string } };
  results?: SarifResult[];
}

export interface SarifOptions {
  category: (ruleId: string, message: string) => string;
  /** Severity when the tool gives none beyond SARIF's `level`. */
  severity?: (level: string | undefined, ruleId: string) => Severity;
  sensitive?: boolean;
}

/**
 * Tools print logs beside their output (ktlint writes a WARN line before the
 * JSON), so the document is whatever starts at the first line beginning with `{`.
 */
export function jsonFrom(text: string): unknown {
  const start = /^\{/m.exec(text)?.index ?? text.indexOf("{");
  if (start < 0) throw new Error("no JSON document in the output");
  return JSON.parse(text.slice(start));
}

/** The tool's version when it says so in the report, so it need not be run twice. */
export function sarifVersion(text: string): string | undefined {
  try {
    const run = (jsonFrom(text) as { runs?: SarifRun[] }).runs?.[0]?.tool?.driver;
    return run?.semanticVersion?.replace(/^v/, "") ?? run?.version;
  } catch {
    return undefined;
  }
}

/** SARIF `level` mapped onto the severity rubric; a style tool's `error` is a rule it enforces, not a defect. */
export function severityFromLevel(level: string | undefined): Severity {
  switch (level) {
    case "error":
      return "medium";
    case "note":
    case "none":
      return "informational";
    default:
      return "low";
  }
}

/** CVSS-style scores (`security-severity`) onto the rubric. */
function severityFromScore(score: number): Severity {
  if (score >= 9) return "critical";
  if (score >= 7) return "high";
  if (score >= 4) return "medium";
  return "low";
}

/** Reads any SARIF 2.1.0 document; the per-tool parsers only decide category and severity. */
export function parseSarif(
  text: string,
  context: ParseContext,
  options: SarifOptions,
): AnalyzerFinding[] {
  const document = jsonFrom(text) as { runs?: SarifRun[] };
  const findings: AnalyzerFinding[] = [];

  for (const run of document.runs ?? []) {
    const rules = new Map<string, SarifRule>();
    for (const rule of run.tool?.driver?.rules ?? []) if (rule.id) rules.set(rule.id, rule);

    for (const result of run.results ?? []) {
      const location = result.locations?.[0]?.physicalLocation;
      const uri = location?.artifactLocation?.uri;
      if (!uri) continue;
      const file = toRepositoryPath(context.rootDir, uri);
      if (!file) continue;

      const ruleId = result.ruleId ?? "unknown";
      const rule = rules.get(ruleId);
      const message = result.message?.text ?? ruleId;
      const score = Number(rule?.properties?.["security-severity"]);
      const severity = Number.isFinite(score)
        ? severityFromScore(score)
        : (options.severity ?? severityFromLevel)(result.level, ruleId);

      const region = location?.region;
      findings.push({
        ruleId,
        message,
        severity,
        category: options.category(ruleId, message),
        file,
        ...(region?.startLine !== undefined ? { line: region.startLine } : {}),
        ...(region?.startColumn !== undefined ? { column: region.startColumn } : {}),
        ...(region?.endLine !== undefined ? { endLine: region.endLine } : {}),
        ...(region?.endColumn !== undefined ? { endColumn: region.endColumn } : {}),
        ...(rule?.helpUri ? { url: rule.helpUri } : {}),
        ...(options.sensitive ? { sensitive: true } : {}),
      });
    }
  }
  return findings;
}
