import type { Comment } from "./lex.js";

/**
 * Inline suppression: a comment that says, in the code itself, that a finding
 * is accepted and why. It reads the way a person would write it:
 *
 *   risky(); // debuggatha-ignore no-eval -- sandboxed plugin loader
 *   // debuggatha-ignore-next-line eqeqeq, no-var -- legacy vendored shim
 *   // debuggatha-ignore-file no-explicit-any -- generated API types
 *
 * With no rule ids the directive covers every rule on its target.
 */

export type DirectiveScope = "line" | "next-line" | "file";

export interface Directive {
  scope: DirectiveScope;
  /** Line of the comment, 1-based, in the reviewed text. */
  line: number;
  /** `undefined` covers every rule. */
  ruleIds: string[] | undefined;
  reason: string | undefined;
}

/** What an inline directive hid, reported alongside findings so nothing is silently dropped. */
export interface InlineSuppression {
  file: string;
  line: number | undefined;
  ruleId: string;
  reason: string | undefined;
}

const DIRECTIVE = /debuggatha-ignore(-next-line|-file)?\b(.*)$/;
const RULE_ID = /^[\w*-]+$/;

export function parseDirectives(comments: readonly Comment[]): Directive[] {
  const directives: Directive[] = [];
  for (const comment of comments) {
    const match = DIRECTIVE.exec(comment.text);
    if (!match) continue;
    const suffix = match[1];
    const scope: DirectiveScope =
      suffix === "-next-line" ? "next-line" : suffix === "-file" ? "file" : "line";

    const [rulePart = "", ...reasonParts] = (match[2] ?? "").split(/\s--\s|\s—\s/);
    const reason =
      reasonParts
        .join(" -- ")
        .replace(/\*\/\s*$/, "")
        .trim() || undefined;
    const ids = rulePart
      .split(/[\s,:]+/)
      .map((id) => id.trim())
      .filter((id) => RULE_ID.test(id));

    directives.push({
      scope,
      line: comment.line,
      ruleIds: ids.length === 0 || ids.includes("*") ? undefined : ids,
      reason,
    });
  }
  return directives;
}

const HASH_COMMENT = /\.(py|rb|sh|ya?ml|toml|env)$/i;
const HTML_COMMENT = /\.(html?|vue|svelte|md)$/i;

/**
 * The comment that accepts one rule on the line after it, in the syntax of the
 * file's language. The reason is required text, not decoration: an accepted
 * finding with no why is how a suppression list turns into a graveyard.
 */
export function inlineSuppressionComment(file: string, ruleId: string, reason: string): string {
  const body = `debuggatha-ignore-next-line ${ruleId} -- ${reason}`;
  if (HASH_COMMENT.test(file)) return `# ${body}`;
  if (HTML_COMMENT.test(file)) return `<!-- ${body} -->`;
  return `// ${body}`;
}

/** The directive that covers `ruleId` on `line` (1-based), if any. */
export function findDirective(
  directives: readonly Directive[],
  line: number,
  ruleId: string,
): Directive | undefined {
  return directives.find((directive) => {
    if (directive.ruleIds && !directive.ruleIds.includes(ruleId)) return false;
    if (directive.scope === "file") return true;
    if (directive.scope === "line") return directive.line === line;
    return directive.line + 1 === line;
  });
}
