import type { LineRange } from "@debuggatha/review-engine";

/**
 * One hit a detector finds in a chunk of source text. Line numbers are
 * relative to whatever text was handed to the detector — the caller
 * (file/diff/architecture skill) is responsible for mapping that back to
 * real file line numbers, since a diff review only ever sees the changed
 * lines, not the whole file.
 */
export interface DetectorHit {
  lines: LineRange | undefined;
  excerpt: string;
}

export interface DetectorInput {
  /** File path, used only so a detector can skip itself on the wrong extension. */
  file: string;
  /** The text to scan — a whole file for File/Architecture Review, or just the added lines for Diff Review. */
  content: string;
  /** Maps each 1-indexed line of `content` to its real line number in the file, when known (diff hunks are not contiguous from line 1). */
  lineNumbers: number[] | undefined;
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

/**
 * Scans line-by-line with a regex, mapping each match back to a real
 * line number via `lineNumbers` (falls back to 1-indexed position in
 * `content` when the caller didn't supply a mapping — the File/
 * Architecture Review case, where `content` already is the whole file).
 */
function scanLines(input: DetectorInput, pattern: RegExp, guard?: (file: string) => boolean) {
  if (guard && !guard(input.file)) return [];
  const hits: DetectorHit[] = [];
  const lines = input.content.split("\n");
  lines.forEach((line, index) => {
    if (!pattern.test(line)) return;
    const line1 = realLine(input.lineNumbers, index);
    hits.push({
      lines: line1 === undefined ? undefined : { start: line1, end: line1 },
      excerpt: line.trim().slice(0, 200),
    });
  });
  return hits;
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
 */
export const RULE_DETECTORS: Record<string, RuleDetector> = {
  "no-explicit-any": (input) => scanLines(input, /:\s*any\b|<any>|as\s+any\b/, isTsLike),
  "strict-null-checks": (input) => scanLines(input, /[A-Za-z0-9_\])]!(?!=)/, isTsLike),
  "no-var": (input) => scanLines(input, /^\s*var\s+[A-Za-z_$]/, isJsLike),
  eqeqeq: (input) => scanLines(input, /[^=!<>]==[^=]|[^!]!=[^=]/, isJsLike),
  "no-eval": (input) =>
    scanLines(input, /\beval\s*\(|setTimeout\s*\(\s*["']|setInterval\s*\(\s*["']/, isJsLike),
  "prefer-template-literals": (input) =>
    scanLines(input, /["'][^"']*["']\s*\+\s*[A-Za-z_$]|[A-Za-z_$][\w.]*\s*\+\s*["']/, isJsLike),
  "no-extend-native": (input) =>
    scanLines(
      input,
      /\b(Object|Array|String|Number|Boolean|Function)\.prototype\.[A-Za-z_$]\w*\s*=/,
      isJsLike,
    ),
  "no-implied-eval": (input) => scanLines(input, /new\s+Function\s*\(/, isJsLike),
  "prefer-object-spread": (input) => scanLines(input, /Object\.assign\s*\(/, isJsLike),
  "avoid-print": (input) => scanLines(input, /\bprint\s*\(/, (file) => file.endsWith(".dart")),
  "no-hardcoded-secrets": (input) =>
    scanLines(input, /(api[_-]?key|secret|password|token)\s*[:=]\s*["'][A-Za-z0-9+/_-]{8,}["']/i),

  // Epic 16B (Deferred Inventory item G): a modest, targeted expansion —
  // not full coverage of every stack pack (explicitly out of scope for
  // this resolution pass, see the package README) — covering a handful
  // of high-confidence, low-false-positive rules from the
  // highest-traffic packs the Deferred Inventory named (React, Vue,
  // Rust, Go).
  "no-dangerously-set-inner-html": (input) =>
    scanLines(input, /dangerouslySetInnerHTML/, (file) => /\.(tsx|jsx)$/.test(file)),
  "use-v-html-carefully": (input) =>
    scanLines(input, /v-html\s*=/, (file) => file.endsWith(".vue")),
  "no-unwrap-expect": (input) =>
    scanLines(input, /\.unwrap\s*\(\s*\)|\.expect\s*\(/, (file) => file.endsWith(".rs")),
  "no-unsafe-blocks": (input) =>
    scanLines(input, /\bunsafe\s*\{|\bunsafe\s+fn\b/, (file) => file.endsWith(".rs")),
  // Shared by both `debuggatha/rust` and `debuggatha/go` — same rule id,
  // different syntax per language (`panic!(...)` vs `panic(...)`),
  // distinguished by file extension rather than needing two registry
  // entries the `Record<string, RuleDetector>` shape can't hold anyway.
  "no-panic": (input) => {
    if (input.file.endsWith(".rs")) return scanLines(input, /\bpanic!\s*\(/);
    if (input.file.endsWith(".go")) return scanLines(input, /\bpanic\s*\(/);
    return [];
  },
};

export function detectorFor(ruleId: string): RuleDetector | undefined {
  return RULE_DETECTORS[ruleId];
}
