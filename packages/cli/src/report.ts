import { createHash } from "node:crypto";
import { isAbsolute, relative } from "node:path";
import { type Finding, ruleIdFromEvidence } from "@debuggatha/engine";
import pc from "picocolors";

export const SEVERITY_ORDER = ["informational", "low", "medium", "high", "critical"] as const;
export type SeverityName = (typeof SEVERITY_ORDER)[number];

export type ReportFormat = "text" | "json" | "sarif" | "github" | "markdown" | "gitlab";
export const REPORT_FORMATS: readonly ReportFormat[] = [
  "text",
  "json",
  "sarif",
  "github",
  "markdown",
  "gitlab",
];

export function severityRank(severity: string): number {
  return SEVERITY_ORDER.indexOf(severity as SeverityName);
}

/** The rule a finding traces back to: the pack rule or repository criterion named in its evidence. */
export function ruleIdOf(finding: Finding): string {
  return ruleIdFromEvidence(finding.evidence) ?? finding.category;
}

/** The external tool that reported a finding, with its version and license; `undefined` for a built-in detector. */
export function analyzerOf(finding: Finding) {
  for (const evidence of finding.evidence) {
    if (evidence.kind === "external-analyzer") return evidence;
  }
  return undefined;
}

/** What a language model said about a finding: it raised it (`detection`) or commented on it (`verification`). */
export function semanticOf(finding: Finding) {
  for (const evidence of finding.evidence) {
    if (evidence.kind === "semantic-review") return evidence;
  }
  return undefined;
}

/** Findings carry repo-relative paths; a file outside the repository is absolute and shown relative to `cwd`. */
export function locationOf(finding: Finding, cwd: string) {
  const location = finding.locations[0];
  const file = location
    ? isAbsolute(location.file)
      ? relative(cwd, location.file) || location.file
      : location.file
    : "";
  return {
    file,
    start: location?.lines?.start,
    end: location?.lines?.end,
    column: location?.columns?.start,
    endColumn: location?.columns?.end,
  };
}

/** Findings at or above the threshold; `none` never fails a run. */
export function exceedsThreshold(findings: readonly Finding[], failOn: SeverityName | "none") {
  if (failOn === "none") return false;
  const floor = severityRank(failOn);
  return findings.some((finding) => severityRank(finding.severity) >= floor);
}

const SEVERITY_COLOR: Record<SeverityName, (text: string) => string> = {
  critical: pc.red,
  high: pc.red,
  medium: pc.yellow,
  low: pc.cyan,
  informational: pc.gray,
};

export function formatText(findings: readonly Finding[], cwd: string): string {
  const sorted = [...findings].sort((a, b) => severityRank(b.severity) - severityRank(a.severity));
  return sorted
    .map((finding) => {
      const { file, start, column } = locationOf(finding, cwd);
      const color = SEVERITY_COLOR[finding.severity as SeverityName] ?? pc.white;
      const where =
        start === undefined
          ? file
          : column === undefined
            ? `${file}:${start}`
            : `${file}:${start}:${column}`;
      const tool = analyzerOf(finding);
      const via = tool
        ? ` · ${tool.tool}${tool.version ? ` ${tool.version}` : ""}, ${tool.license}`
        : "";
      const model = semanticOf(finding);
      const who = model
        ? ` · model suspicion, ${model.provider}${model.model ? ` ${model.model}` : ""}`
        : "";
      const headline = `${color(finding.severity.padEnd(13))} ${where}  ${finding.title}  ${pc.dim(`(${ruleIdOf(finding)}${via}${model?.role === "detection" ? who : ""})`)}`;
      const fix = finding.recommendations[0]?.summary;
      const lines = [headline];
      if (fix) lines.push(`${" ".repeat(14)}${pc.dim(`→ ${fix}`)}`);
      if (model?.role === "verification" && model.verdict) {
        const said = {
          confirmed: "thinks this is real",
          doubtful: "doubts this",
          unsure: "is unsure",
        }[model.verdict];
        lines.push(
          `${" ".repeat(14)}${pc.dim(`model (${model.provider}${model.model ? ` ${model.model}` : ""}) ${said}: ${model.reason}`)}`,
        );
      }
      return lines.join("\n");
    })
    .join("\n");
}

const GITHUB_LEVEL: Record<SeverityName, "error" | "warning" | "notice"> = {
  critical: "error",
  high: "error",
  medium: "warning",
  low: "notice",
  informational: "notice",
};

const escapeData = (value: string) =>
  value.replaceAll("%", "%25").replaceAll("\r", "%0D").replaceAll("\n", "%0A");
const escapeProperty = (value: string) =>
  escapeData(value).replaceAll(":", "%3A").replaceAll(",", "%2C");

/** GitHub Actions workflow commands: each finding becomes an inline annotation on the pull request. */
export function formatGithub(findings: readonly Finding[], cwd: string): string {
  return findings
    .map((finding) => {
      const { file, start, end, column, endColumn } = locationOf(finding, cwd);
      const level = GITHUB_LEVEL[finding.severity as SeverityName] ?? "warning";
      const props = [`file=${escapeProperty(file)}`];
      if (start !== undefined) props.push(`line=${start}`, `endLine=${end ?? start}`);
      if (column !== undefined && endColumn !== undefined) {
        props.push(`col=${column}`, `endColumn=${endColumn}`);
      }
      props.push(`title=${escapeProperty(`${finding.title} (${ruleIdOf(finding)})`)}`);
      return `::${level} ${props.join(",")}::${escapeData(finding.explanation)}`;
    })
    .join("\n");
}

const GITLAB_SEVERITY: Record<SeverityName, string> = {
  informational: "info",
  low: "minor",
  medium: "major",
  high: "critical",
  critical: "blocker",
};

/**
 * GitLab's Code Quality report (`artifacts:reports:codequality`), which a merge request shows as a
 * widget with the new and fixed findings. The fingerprint is what GitLab uses to tell them apart
 * between pipelines, so it is built from what identifies a finding and never from a code excerpt.
 */
export function formatGitlab(findings: readonly Finding[], cwd: string): string {
  return JSON.stringify(
    findings.map((finding) => {
      const { file, start } = locationOf(finding, cwd);
      const rule = ruleIdOf(finding);
      return {
        description: finding.title,
        check_name: rule,
        fingerprint: createHash("sha256")
          .update([rule, file, start ?? "", finding.title].join("\0"))
          .digest("hex")
          .slice(0, 32),
        severity: GITLAB_SEVERITY[finding.severity as SeverityName] ?? "major",
        location: { path: file.replaceAll("\\", "/"), lines: { begin: start ?? 1 } },
      };
    }),
    null,
    2,
  );
}

/** Marks a comment as this report's, so a CI job can find it and update it in place instead of adding another. */
export const MARKDOWN_MARKER = "<!-- debuggatha-report -->";

const MARKDOWN_ROWS = 40;

const cell = (value: string) =>
  value.replace(/\r?\n/g, " ").replace(/\|/g, "\\|").replace(/</g, "&lt;");

export interface MarkdownContext {
  /** One line per analyzer or semantic note worth telling a reviewer. */
  notes?: readonly string[];
  /** A sentence about findings that do not count (outside the change, accepted by the baseline). */
  uncounted?: string;
}

/** Where a finding is on GitHub, when the run knows the repository and commit (Actions sets both). */
function blobLink(file: string, line: number | undefined): string | undefined {
  const { GITHUB_SERVER_URL: server, GITHUB_REPOSITORY: repo, GITHUB_SHA: sha } = process.env;
  if (!server || !repo || !sha) return undefined;
  return `${server}/${repo}/blob/${sha}/${file.split("/").map(encodeURIComponent).join("/")}${line === undefined ? "" : `#L${line}`}`;
}

/** A pull-request comment: the counts first, then what to look at, in a table a person can scan. */
export function formatMarkdown(
  findings: readonly Finding[],
  cwd: string,
  context: MarkdownContext = {},
): string {
  const sorted = [...findings].sort((a, b) => severityRank(b.severity) - severityRank(a.severity));
  const counts = SEVERITY_ORDER.slice()
    .reverse()
    .map((severity) => [severity, findings.filter((f) => f.severity === severity).length] as const)
    .filter(([, count]) => count > 0)
    .map(([severity, count]) => `${count} ${severity}`);

  const out = [MARKDOWN_MARKER, "## Debuggatha review", ""];
  if (findings.length === 0) {
    out.push("No findings.");
  } else {
    out.push(
      `**${findings.length} ${findings.length === 1 ? "finding" : "findings"}**: ${counts.join(", ")}`,
      "",
    );
    out.push("| Severity | Where | Finding | Rule |", "| --- | --- | --- | --- |");
    for (const finding of sorted.slice(0, MARKDOWN_ROWS)) {
      const { file, start } = locationOf(finding, cwd);
      const where = `${file}${start === undefined ? "" : `:${start}`}`;
      const link = blobLink(file, start);
      const tool = analyzerOf(finding);
      const model = semanticOf(finding);
      const rule = `${ruleIdOf(finding)}${tool ? ` (${tool.tool}, ${tool.license})` : ""}${model?.role === "detection" ? " (model suspicion)" : ""}`;
      out.push(
        `| ${finding.severity} | ${link ? `[\`${cell(where)}\`](${link})` : `\`${cell(where)}\``} | ${cell(finding.title)} | ${cell(rule)} |`,
      );
    }
    if (sorted.length > MARKDOWN_ROWS) {
      out.push(
        "",
        `…and ${sorted.length - MARKDOWN_ROWS} more; \`debuggatha review\` lists them all.`,
      );
    }
    const fixes = sorted
      .slice(0, MARKDOWN_ROWS)
      .filter((f) => f.recommendations[0]?.summary)
      .map((f) => {
        const { file, start } = locationOf(f, cwd);
        return `- \`${cell(file)}${start === undefined ? "" : `:${start}`}\`: ${cell(f.recommendations[0]?.summary ?? "")}`;
      });
    if (fixes.length > 0) {
      out.push("", "<details><summary>How to fix</summary>", "", ...fixes, "", "</details>");
    }
  }
  if (context.uncounted) out.push("", context.uncounted);
  for (const note of context.notes ?? []) out.push("", `_${cell(note)}_`);
  return out.join("\n");
}
