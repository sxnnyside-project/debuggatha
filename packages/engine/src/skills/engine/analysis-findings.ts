import { createFinding, type Evidence, type Finding } from '@debuggatha/core';
import { runAnalysis } from '../../analysis-engine/index.js';

/**
 * Turns `engine/analysis-engine`'s deterministic repository analysis
 * into `Finding[]` for Architecture Review — this Skill
 * consumes the Analysis Engine's own dependency graph/layer model
 * instead of re-walking the repository's imports itself rather than duplicating the graph traversal.
 */
export function buildAnalysisFindings(rootDir: string): Finding[] {
  const analysis = runAnalysis(rootDir);
  const findings: Finding[] = [];

  for (const violation of analysis.layerViolations) {
    const evidence: Evidence[] = [
      ...violation.evidence.map(
        (e): Evidence => ({ kind: 'code', file: e.file, lines: undefined, excerpt: e.detail }),
      ),
      {
        kind: 'framework-convention',
        framework: 'layered-architecture',
        detail: `"${violation.from}" (${violation.fromLayer}) -> "${violation.to}" (${violation.toLayer})`,
      },
    ];

    findings.push(
      createFinding({
        title: `Layer violation: ${violation.from} -> ${violation.to}`,
        explanation: `This finding exists because ${violation.explanation}`,
        severity: 'high',
        confidence: 'medium',
        category: 'architecture',
        locations: [{ file: violation.from, lines: undefined }],
        evidence,
        recommendations: [
          {
            id: `layer-violation-${violation.from}-${violation.to}`,
            action: 'refactor',
            summary: `Invert or remove the dependency from "${violation.from}" to "${violation.to}".`,
            rationale: violation.explanation,
            targetFile: violation.from,
            targetLines: undefined,
          },
        ],
      }),
    );
  }

  for (const cycle of analysis.cycles) {
    const evidence: Evidence[] = [
      {
        kind: 'framework-convention',
        framework: 'dependency-graph',
        detail: `Cycle: ${cycle.shortestCycle.join(' -> ')}`,
      },
    ];
    const anchor = cycle.modules[0];
    if (!anchor) continue;

    findings.push(
      createFinding({
        title: `Dependency cycle: ${cycle.modules.join(', ')}`,
        explanation: `This finding exists because ${cycle.modules.length} modules (${cycle.modules.join(', ')}) form a dependency cycle — shortest path: ${cycle.shortestCycle.join(' -> ')}.`,
        severity: cycle.impact === 'high' ? 'high' : cycle.impact === 'medium' ? 'medium' : 'low',
        confidence: 'high',
        category: 'architecture',
        locations: cycle.modules.map((moduleId) => ({ file: moduleId, lines: undefined })),
        evidence,
        recommendations: [
          {
            id: `cycle-${cycle.modules.join('-')}`,
            action: 'refactor',
            summary:
              "Break the dependency cycle by extracting the shared concern into a module none of the cycle's members depend on the others through.",
            rationale: `Modules ${cycle.modules.join(', ')} depend on each other cyclically, which makes them impossible to build, test, or reason about independently.`,
            targetFile: anchor,
            targetLines: undefined,
          },
        ],
      }),
    );
  }

  for (const drift of analysis.envDrift) {
    const first = drift.locations[0];
    if (!first) continue;
    findings.push(
      createFinding({
        title: `Environment variable ${drift.variable} is missing from ${drift.exampleFile}`,
        explanation: `This finding exists because the code reads process.env.${drift.variable} but ${drift.exampleFile} does not list it, so a new environment is configured without it and fails when that code runs.`,
        severity: 'medium',
        confidence: 'medium',
        category: 'reliability',
        locations: drift.locations.map((l) => ({
          file: l.file,
          lines: { start: l.line, end: l.line },
        })),
        evidence: [
          ...drift.locations.map(
            (l): Evidence => ({
              kind: 'code',
              file: l.file,
              lines: { start: l.line, end: l.line },
              excerpt: l.excerpt,
            }),
          ),
          {
            kind: 'framework-convention',
            framework: 'dotenv',
            detail: `${drift.exampleFile} is the repository's list of required environment variables`,
          },
        ],
        recommendations: [
          {
            id: `env-drift-${drift.variable}`,
            action: 'configure',
            summary: `Add ${drift.variable} to ${drift.exampleFile}, or give the read a default if it is optional.`,
            rationale: `${drift.exampleFile} documents what a deployment must set.`,
            targetFile: drift.exampleFile,
            targetLines: undefined,
          },
        ],
      }),
    );
  }

  return findings;
}
