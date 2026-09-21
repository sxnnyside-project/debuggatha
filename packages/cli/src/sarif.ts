import type { Finding } from "@debuggatha/engine";
import pkg from "../package.json" with { type: "json" };
import { analyzerOf, locationOf, ruleIdOf, type SeverityName, semanticOf } from "./report.js";

const SARIF_LEVEL: Record<SeverityName, "error" | "warning" | "note"> = {
  critical: "error",
  high: "error",
  medium: "warning",
  low: "note",
  informational: "note",
};

/** SARIF 2.1.0, the format GitHub code scanning and most CI security dashboards ingest. */
export function toSarif(findings: readonly Finding[], cwd: string) {
  const rules = new Map<
    string,
    {
      id: string;
      name: string;
      shortDescription: { text: string };
      help?: { text: string };
      properties: { precision: string };
    }
  >();
  const results = findings.map((finding) => {
    const ruleId = ruleIdOf(finding);
    if (!rules.has(ruleId)) {
      const recommendation = finding.recommendations[0];
      const example = recommendation?.example
        ? `\n\nBefore:\n${recommendation.example.before}\n\nAfter:\n${recommendation.example.after}`
        : "";
      rules.set(ruleId, {
        id: ruleId,
        name: ruleId,
        shortDescription: { text: finding.title },
        ...(recommendation ? { help: { text: `${recommendation.summary}${example}` } } : {}),
        properties: { precision: finding.confidence },
      });
    }
    const { file, start, end, column, endColumn } = locationOf(finding, cwd);
    const analyzer = analyzerOf(finding);
    const semantic = semanticOf(finding);
    return {
      ruleId,
      level: SARIF_LEVEL[finding.severity as SeverityName] ?? "warning",
      message: { text: `${finding.title}. ${finding.explanation}` },
      locations: [
        {
          physicalLocation: {
            artifactLocation: { uri: file.replaceAll("\\", "/"), uriBaseId: "%SRCROOT%" },
            ...(start === undefined
              ? {}
              : {
                  region: {
                    startLine: start,
                    endLine: end ?? start,
                    ...(column === undefined || endColumn === undefined
                      ? {}
                      : { startColumn: column, endColumn }),
                  },
                }),
          },
        },
      ],
      properties: {
        severity: finding.severity,
        confidence: finding.confidence,
        category: finding.category,
        ...(analyzer
          ? {
              analyzer: {
                tool: analyzer.tool,
                ruleId: analyzer.ruleId,
                license: analyzer.license,
                ...(analyzer.version ? { version: analyzer.version } : {}),
                ...(analyzer.url ? { url: analyzer.url } : {}),
              },
            }
          : {}),
        ...(semantic
          ? {
              semantic: {
                role: semantic.role,
                provider: semantic.provider,
                ...(semantic.model ? { model: semantic.model } : {}),
                ...(semantic.verdict ? { verdict: semantic.verdict } : {}),
                reason: semantic.reason,
              },
            }
          : {}),
      },
    };
  });

  return {
    $schema: "https://json.schemastore.org/sarif-2.1.0.json",
    version: "2.1.0",
    runs: [
      {
        tool: {
          driver: {
            name: "Debuggatha",
            version: pkg.version,
            informationUri: "https://github.com/sxnnyside-project/debuggatha",
            rules: [...rules.values()],
          },
        },
        results,
      },
    ],
  };
}
