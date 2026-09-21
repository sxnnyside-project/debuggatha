import { isAbsolute, join } from "node:path";
import {
  type Finding,
  inlineSuppressionComment,
  type LedgerEntry,
  ruleIdFromEvidence,
} from "@debuggatha/engine";

/*
 * What a finding looks like in the editor, as data. Nothing here imports `vscode`, so it is
 * tested without an editor; the providers only turn these values into VS Code objects.
 */

export interface Span {
  /** 0-based, as the editor counts. */
  line: number;
  startChar: number;
  /** `undefined`: to the end of the line, whatever its length. */
  endChar: number | undefined;
}

/** Where a finding is, precisely when the detector knew the columns and on the whole line when it did not. */
export function spanOf(finding: Finding): Span {
  const location = finding.locations[0];
  const line = Math.max(0, (location?.lines?.start ?? 1) - 1);
  const columns = location?.columns;
  // Columns are 1-based; `end` is just past the match. A range across lines has no usable end column.
  const oneLine = (location?.lines?.end ?? location?.lines?.start) === location?.lines?.start;
  if (columns && oneLine) {
    return {
      line,
      startChar: Math.max(0, columns.start - 1),
      endChar: Math.max(columns.start, columns.end) - 1,
    };
  }
  return { line, startChar: 0, endChar: undefined };
}

export function absolutePath(rootDir: string, file: string): string {
  return isAbsolute(file) ? file : join(rootDir, file);
}

export type Provenance =
  | { kind: "built-in"; pack: string | undefined }
  | {
      kind: "analyzer";
      tool: string;
      version: string | undefined;
      license: string;
      url: string | undefined;
    }
  | { kind: "model"; provider: string; model: string | undefined };

export function provenanceOf(finding: Finding): Provenance {
  for (const evidence of finding.evidence) {
    if (evidence.kind === "external-analyzer") {
      return {
        kind: "analyzer",
        tool: evidence.tool,
        version: evidence.version,
        license: evidence.license,
        url: evidence.url,
      };
    }
    if (evidence.kind === "semantic-review" && evidence.role === "detection") {
      return { kind: "model", provider: evidence.provider, model: evidence.model };
    }
  }
  const pack = finding.evidence.find((evidence) => evidence.kind === "review-pack-rule");
  return { kind: "built-in", pack: pack?.kind === "review-pack-rule" ? pack.packId : undefined };
}

export const ruleOf = (finding: Finding): string =>
  ruleIdFromEvidence(finding.evidence) ?? finding.category;

/** The suggested change, when the finding carries one. */
export function fixOf(finding: Finding) {
  const recommendation = finding.recommendations[0];
  return recommendation
    ? {
        summary: recommendation.summary,
        example: recommendation.example,
        edit: recommendation.edit,
      }
    : undefined;
}

const excerptOf = (finding: Finding): string | undefined => {
  const code = finding.evidence.find((evidence) => evidence.kind === "code");
  return code?.kind === "code" ? code.excerpt : undefined;
};

export interface QuickFixEdit {
  line: number;
  startChar: number;
  endChar: number;
  replacement: string;
  safety: "safe" | "review";
}

/**
 * The exact edit for a finding, but only while the line still says what it said when the review
 * read it. The review is a snapshot: applying a column-exact edit to a line that has since
 * changed would corrupt it, so a stale finding offers no fix until the file is reviewed again.
 */
export function quickFixEdit(
  finding: Finding,
  currentLine: string | undefined,
): QuickFixEdit | undefined {
  const edit = fixOf(finding)?.edit;
  const excerpt = excerptOf(finding);
  if (!edit || currentLine === undefined || excerpt === undefined) return undefined;
  if (currentLine.trim().slice(0, 200) !== excerpt) return undefined;
  return {
    line: edit.line - 1,
    startChar: edit.startColumn - 1,
    endChar: edit.endColumn - 1,
    replacement: edit.replacement,
    safety: edit.safety,
  };
}

/** Only Debuggatha's own detectors read `debuggatha-ignore`; an analyzer's finding has to be dismissed in the ledger. */
export function canSuppressInline(finding: Finding): boolean {
  return finding.evidence.some(
    (evidence) => evidence.kind === "review-pack-rule" || evidence.kind === "criteria",
  );
}

/** The comment line that accepts `finding` on the line below it, indented like the code it covers. */
export function suppressionEdit(finding: Finding, lineText: string, reason: string) {
  const file = finding.locations[0]?.file ?? "";
  const indent = /^\s*/.exec(lineText)?.[0] ?? "";
  return {
    line: Math.max(0, (finding.locations[0]?.lines?.start ?? 1) - 1),
    text: `${indent}${inlineSuppressionComment(file, ruleOf(finding), reason.trim())}\n`,
  };
}

const escapeHtml = (text: string) => text.replace(/[<>]/g, (c) => (c === "<" ? "&lt;" : "&gt;"));

export const commandLink = (title: string, command: string, ...args: unknown[]) =>
  `[${title}](command:${command}?${encodeURIComponent(JSON.stringify(args))})`;

/** The hover for one finding: what it is, where the claim comes from, why it matters, and what to do. */
export function hoverMarkdown(entry: LedgerEntry): string {
  const finding = entry.latestFinding;
  const provenance = provenanceOf(finding);
  const fix = fixOf(finding);
  const lines: string[] = [];

  lines.push(`**${finding.severity}** · ${escapeHtml(finding.title)}`);

  const rule = `\`${ruleOf(finding)}\``;
  if (provenance.kind === "analyzer") {
    const version = provenance.version ? ` ${provenance.version}` : "";
    const docs = provenance.url ? ` · [documentation](${provenance.url})` : "";
    lines.push(`${rule} · ${provenance.tool}${version}, ${provenance.license}${docs}`);
  } else if (provenance.kind === "model") {
    lines.push(
      `${rule} · a suspicion raised by a model (${provenance.provider}${provenance.model ? ` ${provenance.model}` : ""}), not a rule: check it before acting`,
    );
  } else {
    lines.push(`${rule}${provenance.pack ? ` · ${provenance.pack}` : ""}`);
  }
  lines.push(`_confidence: ${finding.confidence} · ${entry.status}_`);

  // An analyzer's explanation is boilerplate around its own message; the title already carries it.
  if (provenance.kind !== "analyzer" && finding.explanation) {
    lines.push("", escapeHtml(finding.explanation));
  }
  if (fix) {
    lines.push("", `**Fix:** ${escapeHtml(fix.summary)}`);
    if (fix.example) {
      lines.push("", "```", `// before`, fix.example.before, `// after`, fix.example.after, "```");
    }
  }

  const actions = [
    commandLink("Open detail", "debuggatha.openFindingById", entry.id),
    commandLink("Resolve", "debuggatha.resolveFindingById", entry.id),
    commandLink("Dismiss", "debuggatha.dismissFindingById", entry.id),
    ...(canSuppressInline(finding)
      ? [commandLink("Suppress in code…", "debuggatha.suppressInline", entry.id)]
      : []),
  ];
  lines.push("", "---", actions.join(" · "));
  return lines.join("\n");
}
