import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { type Finding, ruleIdFromEvidence } from "@debuggatha/core";
import { executeReview } from "../../pipeline.js";
import { detectionPrompt, detectionsFrom, judge, windowAround } from "../judge.js";
import type { SemanticProvider } from "../types.js";
import {
  DETECTION,
  type DetectionCase,
  type Stack,
  VERIFICATION,
  type VerificationCase,
} from "./dataset.js";

/**
 * Reference marks for lightweight models, not a pass/fail gate. They say where a small local
 * model sits against a bar worth aiming at: doubting a real finding hides a real problem, so the
 * verification marks are about not being wrong when a model says "false positive", and a small
 * model will not detect much, so detection is judged mostly on not raising false alarms.
 */
export const BENCHMARK = {
  verification: {
    /** Of the findings the model doubts, the share that really are false positives. */
    doubtPrecision: 0.8,
    /** Of the real findings, the share the model did not doubt. */
    realRetained: 0.9,
  },
  detection: {
    /** Of the issues the model raises (after grounding), the share that are planted defects. */
    precision: 0.5,
  },
} as const;

const MANIFESTS: Record<Stack, Record<string, string>> = {
  ts: {
    "package.json": JSON.stringify({
      name: "fixture",
      dependencies: { react: "^18.0.0", express: "^4.0.0" },
      devDependencies: { typescript: "^5.0.0" },
    }),
  },
  python: { "requirements.txt": "flask\n" },
  rust: { "Cargo.toml": '[package]\nname = "fixture"\nversion = "0.1.0"\nedition = "2021"\n' },
  go: { "go.mod": "module fixture\n\ngo 1.22\n" },
};

function makeRepo(stack: Stack, file: string, code: string): string {
  const dir = mkdtempSync(join(tmpdir(), "debuggatha-semantic-eval-"));
  for (const [name, content] of Object.entries(MANIFESTS[stack])) {
    writeFileSync(join(dir, name), content);
  }
  mkdirSync(dirname(join(dir, file)), { recursive: true });
  writeFileSync(join(dir, file), code);
  return dir;
}

/** The finding one of the built-in detectors makes on a case: the real input of verification. */
function builtInFinding(testCase: VerificationCase, dir: string): Finding | undefined {
  const { result } = executeReview({
    rootDir: dir,
    scope: { kind: "files", paths: [join(dir, testCase.file)] },
    depth: "full",
    packIds: [],
    policyId: undefined,
    sourceName: "semantic-eval",
    persist: false,
  });
  const lines = readFileSync(join(dir, testCase.file), "utf8").split("\n");
  return result.findings.find((finding) => {
    if (ruleIdFromEvidence(finding.evidence) !== testCase.rule) return false;
    const line = finding.locations[0]?.lines?.start;
    return line !== undefined && (lines[line - 1] ?? "").includes(testCase.flagged);
  });
}

export interface VerificationOutcome {
  id: string;
  category: string;
  truth: "real" | "false_positive";
  verdict: "confirmed" | "doubtful" | "unsure" | "failed" | "no-finding";
  reason: string;
}

export interface CategoryScores {
  cases: number;
  real: number;
  falsePositive: number;
  confirmed: number;
  doubtful: number;
  unsure: number;
  failed: number;
  /** Doubted correctly / doubted. `undefined` when nothing was doubted. */
  doubtPrecision: number | undefined;
  /** Doubted correctly / all false positives. */
  falsePositiveRecall: number | undefined;
  /** Real findings not doubted / real findings. */
  realRetained: number | undefined;
}

const ratio = (part: number, whole: number) => (whole === 0 ? undefined : part / whole);

export function scoreVerification(outcomes: readonly VerificationOutcome[]): {
  overall: CategoryScores;
  byCategory: Record<string, CategoryScores>;
} {
  const score = (subset: readonly VerificationOutcome[]): CategoryScores => {
    const count = (predicate: (o: VerificationOutcome) => boolean) =>
      subset.filter(predicate).length;
    const doubted = count((o) => o.verdict === "doubtful");
    const doubtedRight = count((o) => o.verdict === "doubtful" && o.truth === "false_positive");
    const real = count((o) => o.truth === "real");
    const falsePositive = count((o) => o.truth === "false_positive");
    return {
      cases: subset.length,
      real,
      falsePositive,
      confirmed: count((o) => o.verdict === "confirmed"),
      doubtful: doubted,
      unsure: count((o) => o.verdict === "unsure"),
      failed: count((o) => o.verdict === "failed" || o.verdict === "no-finding"),
      doubtPrecision: ratio(doubtedRight, doubted),
      falsePositiveRecall: ratio(doubtedRight, falsePositive),
      realRetained: ratio(
        real - count((o) => o.verdict === "doubtful" && o.truth === "real"),
        real,
      ),
    };
  };
  const categories = [...new Set(outcomes.map((o) => o.category))].sort();
  return {
    overall: score(outcomes),
    byCategory: Object.fromEntries(
      categories.map((category) => [
        category,
        score(outcomes.filter((o) => o.category === category)),
      ]),
    ),
  };
}

export async function evaluateVerification(
  provider: SemanticProvider,
  cases: readonly VerificationCase[] = VERIFICATION,
): Promise<VerificationOutcome[]> {
  const outcomes: VerificationOutcome[] = [];
  for (const testCase of cases) {
    const dir = makeRepo(testCase.stack, testCase.file, testCase.code);
    try {
      const finding = builtInFinding(testCase, dir);
      if (!finding) {
        outcomes.push({
          id: testCase.id,
          category: "unknown",
          truth: testCase.truth,
          verdict: "no-finding",
          reason: "no detector finding",
        });
        continue;
      }
      const line = finding.locations[0]?.lines?.start ?? 1;
      const view = windowAround(dir, testCase.file, line);
      const base = { id: testCase.id, category: finding.category, truth: testCase.truth };
      if (!view) {
        outcomes.push({ ...base, verdict: "failed", reason: "file missing" });
        continue;
      }
      try {
        const verification = await judge(provider, finding, view, new Set());
        outcomes.push(
          verification
            ? { ...base, verdict: verification.verdict, reason: verification.reason }
            : { ...base, verdict: "failed", reason: "unusable answer" },
        );
      } catch (error) {
        outcomes.push({ ...base, verdict: "failed", reason: String(error) });
      }
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  }
  return outcomes;
}

export interface DetectionOutcome {
  id: string;
  planted: { category: string; found: boolean }[];
  /** Grounded reports, and how many of them sit on a planted defect. */
  raised: number;
  onPlanted: number;
  /** Claims dropped because the quoted line was not there. */
  ungrounded: number;
  failed: boolean;
}

export interface DetectionScores {
  cases: number;
  planted: number;
  found: number;
  raised: number;
  onPlanted: number;
  falseAlarms: number;
  ungrounded: number;
  failed: number;
  /** Raised issues that sit on a planted defect / raised issues. */
  precision: number | undefined;
  recall: number | undefined;
  byCategory: Record<string, { planted: number; found: number; recall: number | undefined }>;
  /** Issues raised on files that have no planted defect. */
  cleanFileAlarms: number;
}

export async function evaluateDetection(
  provider: SemanticProvider,
  cases: readonly DetectionCase[] = DETECTION,
): Promise<DetectionOutcome[]> {
  const outcomes: DetectionOutcome[] = [];
  for (const testCase of cases) {
    const lines = testCase.code.split("\n");
    const view = {
      file: testCase.file,
      lines: lines.map((text, index) => ({ n: index + 1, text })),
    };
    const plantedLines = testCase.planted.map((defect) => ({
      category: defect.category,
      line: lines.findIndex((text) => text.includes(defect.at)) + 1,
    }));
    try {
      const reply = await provider.complete(detectionPrompt(view, new Set()));
      const { accepted, rejected } = detectionsFrom(reply, view, provider, "eval");
      const hits = (line: number) =>
        plantedLines.some((planted) => Math.abs(planted.line - line) <= 2);
      outcomes.push({
        id: testCase.id,
        planted: plantedLines.map((planted) => ({
          category: planted.category,
          found: accepted.some(
            (f) => Math.abs((f.locations[0]?.lines?.start ?? -99) - planted.line) <= 2,
          ),
        })),
        raised: accepted.length,
        onPlanted: accepted.filter((f) => hits(f.locations[0]?.lines?.start ?? -99)).length,
        ungrounded: rejected,
        failed: false,
      });
    } catch {
      outcomes.push({
        id: testCase.id,
        planted: plantedLines.map((p) => ({ category: p.category, found: false })),
        raised: 0,
        onPlanted: 0,
        ungrounded: 0,
        failed: true,
      });
    }
  }
  return outcomes;
}

export function scoreDetection(outcomes: readonly DetectionOutcome[]): DetectionScores {
  const planted = outcomes.flatMap((o) => o.planted);
  const raised = outcomes.reduce((sum, o) => sum + o.raised, 0);
  const onPlanted = outcomes.reduce((sum, o) => sum + o.onPlanted, 0);
  const byCategory: DetectionScores["byCategory"] = {};
  for (const item of planted) {
    const entry = byCategory[item.category] ?? { planted: 0, found: 0, recall: undefined };
    byCategory[item.category] = entry;
    entry.planted += 1;
    if (item.found) entry.found += 1;
  }
  for (const entry of Object.values(byCategory)) entry.recall = ratio(entry.found, entry.planted);
  const found = planted.filter((item) => item.found).length;
  return {
    cases: outcomes.length,
    planted: planted.length,
    found,
    raised,
    onPlanted,
    falseAlarms: raised - onPlanted,
    ungrounded: outcomes.reduce((sum, o) => sum + o.ungrounded, 0),
    failed: outcomes.filter((o) => o.failed).length,
    precision: ratio(onPlanted, raised),
    recall: ratio(found, planted.length),
    byCategory,
    cleanFileAlarms: outcomes
      .filter((o) => o.planted.length === 0)
      .reduce((sum, o) => sum + o.raised, 0),
  };
}

const pct = (value: number | undefined) =>
  value === undefined ? "  n/a" : `${(value * 100).toFixed(0).padStart(3)}%`;

export function formatReport(
  model: string,
  verification: VerificationOutcome[],
  detection: DetectionOutcome[],
): string {
  const v = scoreVerification(verification);
  const d = scoreDetection(detection);
  const rows = [["overall", v.overall] as const, ...Object.entries(v.byCategory)];
  const out: string[] = [
    `Model: ${model}`,
    "",
    "Verification: does the model recognize false positives without doubting real findings?",
  ];
  out.push(
    "category          cases real  fp  | confirmed doubtful unsure failed | doubt-precision fp-recall real-retained",
  );
  for (const [name, s] of rows) {
    out.push(
      `${name.padEnd(17)} ${String(s.cases).padStart(5)} ${String(s.real).padStart(4)} ${String(s.falsePositive).padStart(3)}  | ${String(s.confirmed).padStart(9)} ${String(s.doubtful).padStart(8)} ${String(s.unsure).padStart(6)} ${String(s.failed).padStart(6)} | ${pct(s.doubtPrecision).padStart(15)} ${pct(s.falsePositiveRecall).padStart(9)} ${pct(s.realRetained).padStart(13)}`,
    );
  }
  const vt = BENCHMARK.verification;
  out.push(
    `Benchmark: doubt-precision >= ${vt.doubtPrecision * 100}% and real-retained >= ${vt.realRetained * 100}%  ->  ${
      (v.overall.doubtPrecision ?? 1) >= vt.doubtPrecision &&
      (v.overall.realRetained ?? 1) >= vt.realRetained
        ? "reached"
        : "below"
    }`,
  );
  out.push("", "Detection: what a model finds in code the analyzers passed.");
  out.push(
    `planted ${d.planted}, found ${d.found} (recall ${pct(d.recall).trim()}), raised ${d.raised}, on a planted defect ${d.onPlanted} (precision ${pct(d.precision).trim()}), false alarms ${d.falseAlarms} (${d.cleanFileAlarms} in clean files), ungrounded and dropped ${d.ungrounded}, failed calls ${d.failed}`,
  );
  for (const [category, s] of Object.entries(d.byCategory)) {
    out.push(
      `  ${category.padEnd(15)} planted ${s.planted}, found ${s.found} (recall ${pct(s.recall).trim()})`,
    );
  }
  const dt = BENCHMARK.detection;
  out.push(
    `Benchmark: precision >= ${dt.precision * 100}%  ->  ${(d.precision ?? 1) >= dt.precision ? "reached" : "below"}`,
  );
  return out.join("\n");
}
