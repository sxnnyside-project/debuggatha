import type { Confidence, LineRange, Severity } from "@debuggatha/core";
import { isTestPath } from "../../scan-scope/index.js";
import type { StringLiteral } from "./lex.js";

/**
 * One hit a detector finds in a chunk of source text. Line numbers are
 * relative to whatever text was handed to the detector — the caller
 * (file/diff/architecture skill) is responsible for mapping that back to
 * real file line numbers, since a diff review only ever sees the changed
 * lines, not the whole file.
 */
export interface DetectorHit {
  /** 0-based line within the reviewed text, for matching inline suppression comments. */
  index: number;
  /** The file's real line numbers. */
  lines: LineRange | undefined;
  /** 1-based columns on the first line: where the match starts and just past its end. */
  columns: LineRange;
  excerpt: string;
  /** How sure the detector is that this is what the rule describes: high = unambiguous syntax, medium = heuristic, low = weak signal. */
  confidence: Confidence;
  /** Overrides the rule's default severity when this particular hit is more or less serious. */
  severity?: Severity;
  /** An exact replacement on this line, for rules whose fix is mechanical. */
  edit?: HitEdit;
}

export interface HitEdit {
  /** 1-based, inclusive. */
  startColumn: number;
  /** 1-based, just past the last replaced character. */
  endColumn: number;
  replacement: string;
  safety: "safe" | "review";
}

export interface DetectorInput {
  /** File path, used only so a detector can skip itself on the wrong extension. */
  file: string;
  /** The text to scan — a whole file for File/Architecture Review, or just the added lines for Diff Review. */
  content: string;
  /** Maps each 1-indexed line of `content` to its real line number in the file, when known (diff hunks are not contiguous from line 1). */
  lineNumbers: number[] | undefined;
  /** `content` with comments and string contents blanked, same layout: what is left is code. */
  masked: string;
  strings: StringLiteral[];
}

export type RuleDetector = (input: DetectorInput) => DetectorHit[];

function isJsLike(file: string): boolean {
  return /\.(ts|tsx|js|jsx|mjs|cjs)$/.test(file);
}

function isTsLike(file: string): boolean {
  return /\.(ts|tsx)$/.test(file);
}

function realLine(lineNumbers: number[] | undefined, index: number): number | undefined {
  if (!lineNumbers) return index + 1;
  return lineNumbers[index];
}

function lineRangeAt(input: DetectorInput, index: number): LineRange | undefined {
  const line = realLine(input.lineNumbers, index);
  return line === undefined ? undefined : { start: line, end: line };
}

// --- Secrets ----------------------------------------------------------------

/** Formats that identify themselves: a match is a credential, not a guess. */
const KNOWN_TOKENS: RegExp[] = [
  /\b(?:AKIA|ASIA)[0-9A-Z]{16}\b/,
  /\bgh[pousr]_[A-Za-z0-9]{36,}\b/,
  /\bgithub_pat_[A-Za-z0-9_]{50,}\b/,
  /\bxox[baprs]-[A-Za-z0-9-]{10,}\b/,
  /\b(?:sk|rk)_live_[A-Za-z0-9]{16,}\b/,
  /\bAIza[0-9A-Za-z_-]{35}\b/,
  /-----BEGIN (?:[A-Z]+ )?PRIVATE KEY-----/,
  /\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b/,
  /\bSG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}\b/,
];

const SECRET_NAME =
  /(?:pass(?:word|wd)?|pwd|secret|token|api[_-]?key|apikey|access[_-]?key|private[_-]?key|credential|auth[_-]?key)/i;
const PASSWORD_NAME = /(?:pass(?:word|wd)?|pwd|secret)/i;
const PLACEHOLDER =
  /^(?:[x*#.\-_]+)$|example|sample|placeholder|change[_-]?me|your[_-]|dummy|fake|mock|redacted|todo|<[^>]*>|\$\{|\{\{|%[sd]/i;

function entropy(value: string): number {
  const counts = new Map<string, number>();
  for (const char of value) counts.set(char, (counts.get(char) ?? 0) + 1);
  let bits = 0;
  for (const count of counts.values()) {
    const p = count / value.length;
    bits -= p * Math.log2(p);
  }
  return bits;
}

/** Words joined by separators (`access_token`, `Invalid password`) are labels and identifiers, not secrets. */
function looksLikeWords(value: string): boolean {
  const parts = value.split(/[_.\-\s]+/).filter(Boolean);
  if (parts.length === 0 || !parts.every((part) => /^[A-Za-z]+$/.test(part))) return false;
  return parts.length > 1 || value.length <= 24;
}

const redactKnownTokens = (line: string) =>
  KNOWN_TOKENS.reduce(
    (text, pattern) => text.replace(new RegExp(pattern.source, "g"), "[redacted]"),
    line,
  );

function excerptOf(rawLine: string): string {
  return redactKnownTokens(rawLine.trim()).slice(0, 200);
}

const ASSIGNED_NAME = /([A-Za-z_][\w.-]*)["']?\s*(?:=>|[:=])\s*$/;

function detectHardcodedSecrets(input: DetectorInput): DetectorHit[] {
  const rawLines = input.content.split("\n");
  const hits: DetectorHit[] = [];
  const flagged = new Set<number>();

  const inTests = isTestPath(input.file);

  // Credentials that name their own format, wherever they appear (even in a comment).
  // In test code the same string is most likely a fixture, so it is reported, but as a smaller problem.
  rawLines.forEach((line, index) => {
    for (const pattern of KNOWN_TOKENS) {
      const match = pattern.exec(line);
      if (!match || /EXAMPLE|SAMPLE|XXXX/i.test(match[0])) continue;
      hits.push({
        index,
        lines: lineRangeAt(input, index),
        columns: { start: match.index + 1, end: match.index + 1 + match[0].length },
        excerpt: excerptOf(line),
        confidence: inTests ? "medium" : "high",
        severity: inTests ? "medium" : "critical",
      });
      flagged.add(index);
      return;
    }
  });

  // A literal assigned to a name that says it is a secret. Test code carries fake credentials by design.
  if (inTests) return hits;

  const lineStarts = [0];
  for (let i = 0; i < input.content.length; i += 1) {
    if (input.content[i] === "\n") lineStarts.push(i + 1);
  }
  let cursor = 0;
  for (const literal of input.strings) {
    while (cursor + 1 < lineStarts.length && (lineStarts[cursor + 1] ?? 0) <= literal.start)
      cursor += 1;
    if (flagged.has(cursor)) continue;
    const column = literal.start - (lineStarts[cursor] ?? 0);
    const before = (rawLines[cursor] ?? "").slice(0, column);
    const name = ASSIGNED_NAME.exec(before)?.[1];
    if (!name || !SECRET_NAME.test(name)) continue;

    const value = literal.value;
    if (value.length < 6 || PLACEHOLDER.test(value) || looksLikeWords(value)) continue;
    if (/^(?:https?:)?\/\//i.test(value) || /^[./~]/.test(value)) continue;
    if (value.toLowerCase() === name.toLowerCase().replace(/[_.-]/g, "")) continue;
    const strong = value.length >= 16 && entropy(value) >= 3;
    if (!PASSWORD_NAME.test(name) && !strong) continue;

    hits.push({
      index: cursor,
      lines: lineRangeAt(input, cursor),
      columns: { start: column + 1, end: column + 1 + (literal.end - literal.start) },
      excerpt: excerptOf((rawLines[cursor] ?? "").replace(value, "[redacted]")),
      confidence: strong ? "medium" : "low",
      severity: "high",
    });
    flagged.add(cursor);
  }
  return hits.sort((a, b) => a.index - b.index);
}

// --- Pattern detectors -------------------------------------------------------

interface ScanOptions {
  guard?: (file: string) => boolean;
  confidence: Confidence;
  /** Rejects a match the pattern alone cannot tell is a false positive. */
  accept?: (match: RegExpMatchArray, line: string) => boolean;
  /** Test code is allowed to be sloppy about this rule. */
  skipTests?: boolean;
  /** The replacement for a match, as offsets within the match, when the fix is mechanical. */
  edit?: (match: RegExpMatchArray) => {
    from: number;
    to: number;
    replacement: string;
    safety: "safe" | "review";
  };
}

/** Scans the masked code line by line, reporting at most one hit per line, with its columns. */
function scanCode(input: DetectorInput, pattern: RegExp, options: ScanOptions): DetectorHit[] {
  if (options.guard && !options.guard(input.file)) return [];
  if (options.skipTests && isTestPath(input.file)) return [];

  const global = new RegExp(
    pattern.source,
    pattern.flags.includes("g") ? pattern.flags : `${pattern.flags}g`,
  );
  const raw = input.content.split("\n");
  const hits: DetectorHit[] = [];
  input.masked.split("\n").forEach((line, index) => {
    for (const match of line.matchAll(global)) {
      if (options.accept && !options.accept(match, line)) continue;
      const start = match.index ?? 0;
      const change = options.edit?.(match);
      hits.push({
        index,
        lines: lineRangeAt(input, index),
        columns: { start: start + 1, end: start + 1 + match[0].length },
        excerpt: excerptOf(raw[index] ?? ""),
        confidence: options.confidence,
        ...(change
          ? {
              edit: {
                startColumn: start + change.from + 1,
                endColumn: start + change.to + 1,
                replacement: change.replacement,
                safety: change.safety,
              },
            }
          : {}),
      });
      break;
    }
  });
  return hits;
}

/** `x == null` is the accepted way to test for null or undefined at once, so it is not a loose-equality bug. */
function isNullComparison(match: RegExpMatchArray, line: string): boolean {
  const start = match.index ?? 0;
  const before = line.slice(0, start);
  const after = line.slice(start + match[0].length);
  return /\bnull\s*$/.test(before) || /^\s*null\b/.test(after);
}

/**
 * Curated, hand-written per rule id — deliberately not "generic" or
 * "AI-generated on the fly": a Rule is declarative data (knowledge-system
 * README, "Rules are declarative, not executable"), and something has to
 * turn "no-explicit-any" into an actual check. This registry is that
 * something. Coverage is intentionally partial (see the package README's
 * "Known limitations") — a resolved `PolicyRule` with no entry here is
 * inspectable in the assembled policy but produces no findings, which is
 * CLAUDE.md's "never fakes certainty" applied to detection coverage, not
 * just to individual findings. Rules that need real type information or
 * structural (AST) analysis to avoid unacceptable false-positive rates
 * (`no-floating-promises`, `consistent-type-imports`, `no-loop-func`,
 * `require-await`) are deliberately left undetected here rather than
 * approximated with a noisy regex — see the same limitations section.
 *
 * Detectors read the masked code, so text inside strings and comments never
 * matches, and every hit carries the columns it spans.
 */
export const RULE_DETECTORS: Record<string, RuleDetector> = {
  "no-explicit-any": (input) =>
    scanCode(input, /:\s*any\b|<any>|as\s+any\b/, {
      guard: isTsLike,
      confidence: "high",
      // `unknown` is the right direction, but callers may rely on `any`, so the result needs checking.
      edit: (match) => ({
        from: match[0].lastIndexOf("any"),
        to: match[0].lastIndexOf("any") + 3,
        replacement: "unknown",
        safety: "review",
      }),
    }),
  "strict-null-checks": (input) =>
    scanCode(input, /[A-Za-z0-9_\])]!(?!=)/, { guard: isTsLike, confidence: "medium" }),
  "no-var": (input) =>
    scanCode(input, /\bvar\s+[A-Za-z_$]/, {
      guard: isJsLike,
      confidence: "high",
      // `let` never changes behavior a `var` in normal code relied on, and `const` is a follow-up.
      edit: () => ({ from: 0, to: 3, replacement: "let", safety: "safe" }),
    }),
  eqeqeq: (input) =>
    scanCode(input, /(?<![=!<>])(?:==|!=)(?!=)/, {
      guard: isJsLike,
      confidence: "high",
      accept: (match, line) => !isNullComparison(match, line),
      edit: (match) => ({
        from: 0,
        to: 2,
        replacement: match[0] === "==" ? "===" : "!==",
        safety: "review",
      }),
    }),
  "no-eval": (input) =>
    scanCode(input, /(?<![.\w$])eval\s*\(|(?<![.\w$])(?:setTimeout|setInterval)\s*\(\s*["'`]/, {
      guard: (file) => isJsLike(file) || file.endsWith(".php"),
      confidence: "high",
    }),
  "prefer-template-literals": (input) =>
    scanCode(input, /["'`][^"'`]*["'`]\s*\+\s*[A-Za-z_$]|[A-Za-z_$][\w.]*\s*\+\s*["'`]/, {
      guard: isJsLike,
      confidence: "low",
    }),
  "no-extend-native": (input) =>
    scanCode(
      input,
      /\b(?:Object|Array|String|Number|Boolean|Function)\.prototype\.[A-Za-z_$]\w*\s*=/,
      { guard: isJsLike, confidence: "high" },
    ),
  "no-implied-eval": (input) =>
    scanCode(input, /(?<![.\w$])new\s+Function\s*\(/, { guard: isJsLike, confidence: "high" }),
  "prefer-object-spread": (input) =>
    scanCode(input, /Object\.assign\s*\(\s*\{\s*\}/, { guard: isJsLike, confidence: "medium" }),
  "avoid-print": (input) =>
    scanCode(input, /(?<![.\w$])print\s*\(/, {
      guard: (file) => file.endsWith(".dart"),
      confidence: "high",
      edit: () => ({ from: 0, to: 5, replacement: "debugPrint", safety: "safe" }),
    }),
  "no-hardcoded-secrets": detectHardcodedSecrets,

  // A handful of high-confidence, low-false-positive rules from the
  // highest-traffic packs (React, Vue, Rust, Go); not every pack rule has a detector.
  "no-dangerously-set-inner-html": (input) =>
    scanCode(input, /dangerouslySetInnerHTML/, {
      guard: (file) => /\.(tsx|jsx)$/.test(file),
      confidence: "high",
    }),
  "use-v-html-carefully": (input) =>
    scanCode(input, /v-html\s*=/, { guard: (file) => file.endsWith(".vue"), confidence: "high" }),
  "no-unwrap-expect": (input) =>
    scanCode(input, /\.unwrap\s*\(\s*\)|\.expect\s*\(/, {
      guard: (file) => file.endsWith(".rs"),
      confidence: "high",
      skipTests: true,
    }),
  "no-unsafe-blocks": (input) =>
    scanCode(input, /\bunsafe\s*\{|\bunsafe\s+fn\b/, {
      guard: (file) => file.endsWith(".rs"),
      confidence: "high",
    }),
  // Shared by both `debuggatha/rust` and `debuggatha/go` — same rule id,
  // different syntax per language (`panic!(...)` vs `panic(...)`),
  // distinguished by file extension rather than needing two registry
  // entries the `Record<string, RuleDetector>` shape can't hold anyway.
  "no-panic": (input) => {
    if (input.file.endsWith(".rs")) {
      return scanCode(input, /\bpanic!\s*\(/, { confidence: "high", skipTests: true });
    }
    if (input.file.endsWith(".go")) {
      return scanCode(input, /(?<![.\w])panic\s*\(/, { confidence: "high", skipTests: true });
    }
    return [];
  },
};

export function detectorFor(ruleId: string): RuleDetector | undefined {
  return RULE_DETECTORS[ruleId];
}
