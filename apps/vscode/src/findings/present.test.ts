import { createFinding, type Finding, type LedgerEntry, loadLedger } from "@debuggatha/engine";
import { afterEach, describe, expect, it } from "vitest";
import { makeRepo, removeRepos, seedLedger } from "../test/fixtures.js";
import {
  canSuppressInline,
  commandLink,
  fixOf,
  hoverMarkdown,
  provenanceOf,
  quickFixEdit,
  ruleOf,
  spanOf,
  suppressionEdit,
} from "./present.js";

afterEach(removeRepos);

/** Real findings from a real review of code the built-in detectors flag. */
function realEntries(source?: string): { entries: LedgerEntry[]; lines: string[] } {
  const { root } = makeRepo(source);
  seedLedger(root);
  const source_ = source ?? "";
  return { entries: [...loadLedger(root).entries], lines: source_.split("\n") };
}

const byRule = (entries: LedgerEntry[], rule: string) => {
  const entry = entries.find((e) => ruleOf(e.latestFinding) === rule);
  if (!entry) throw new Error(`no ${rule} finding`);
  return entry;
};

const VAR_LINE = "var count = 1;";

describe("spanOf", () => {
  it("covers exactly the text the detector matched, when it knows the columns", () => {
    const { entries } = realEntries("var x: any = 1;\n");
    const span = spanOf(byRule(entries, "no-explicit-any").latestFinding);
    // `: any` starts at the colon, on the first line.
    expect(span.line).toBe(0);
    expect("var x: any = 1;".slice(span.startChar, span.endChar)).toBe(": any");
  });

  it("falls back to the whole line when there are no columns", () => {
    const finding = createFinding({
      title: "t",
      explanation: "e",
      severity: "low",
      confidence: "high",
      category: "maintainability",
      locations: [{ file: "a.ts", lines: { start: 7, end: 7 } }],
      evidence: [{ kind: "code", file: "a.ts", lines: undefined, excerpt: undefined }],
    });
    expect(spanOf(finding)).toEqual({ line: 6, startChar: 0, endChar: undefined });
  });

  it("a finding across lines is the whole first line, not a column range from another line", () => {
    const finding = createFinding({
      title: "t",
      explanation: "e",
      severity: "low",
      confidence: "high",
      category: "maintainability",
      locations: [{ file: "a.ts", lines: { start: 3, end: 9 }, columns: { start: 5, end: 2 } }],
      evidence: [{ kind: "code", file: "a.ts", lines: undefined, excerpt: undefined }],
    });
    expect(spanOf(finding)).toEqual({ line: 2, startChar: 0, endChar: undefined });
  });
});

describe("quickFixEdit", () => {
  it("is the engine's exact edit while the line still reads as it did at review time", () => {
    const { entries } = realEntries(`${VAR_LINE}\n`);
    const finding = byRule(entries, "no-var").latestFinding;
    const fix = quickFixEdit(finding, VAR_LINE);
    expect(fix).toMatchObject({
      line: 0,
      startChar: 0,
      endChar: 3,
      replacement: "let",
      safety: "safe",
    });
    // Applying it to the line gives valid code.
    expect(
      VAR_LINE.slice(0, fix?.startChar) + fix?.replacement + VAR_LINE.slice(fix?.endChar),
    ).toBe("let count = 1;");
  });

  it("follows an indented line: the excerpt is the trimmed line", () => {
    const { entries } = realEntries(`function f() {\n    ${VAR_LINE}\n}\n`);
    const finding = byRule(entries, "no-var").latestFinding;
    expect(quickFixEdit(finding, `    ${VAR_LINE}`)).toBeDefined();
  });

  it("offers nothing once the line has changed, because a column-exact edit would corrupt it", () => {
    const { entries } = realEntries(`${VAR_LINE}\n`);
    const finding = byRule(entries, "no-var").latestFinding;
    expect(quickFixEdit(finding, "var count = 2; // edited")).toBeUndefined();
    expect(quickFixEdit(finding, undefined)).toBeUndefined();
    expect(quickFixEdit(finding, "")).toBeUndefined();
  });

  it("a finding with no exact edit has none to offer", () => {
    const { entries } = realEntries('var x: any = 1;\nif (x == 2) { eval("1"); }\n');
    const finding = byRule(entries, "no-eval").latestFinding;
    expect(fixOf(finding)?.edit).toBeUndefined();
    expect(quickFixEdit(finding, 'if (x == 2) { eval("1"); }')).toBeUndefined();
  });
});

describe("suppression", () => {
  it("only Debuggatha's own detectors can be silenced by a comment", () => {
    const { entries } = realEntries(`${VAR_LINE}\n`);
    expect(canSuppressInline(byRule(entries, "no-var").latestFinding)).toBe(true);

    const analyzer = createFinding({
      title: "unused import",
      explanation: "x",
      severity: "low",
      confidence: "high",
      category: "maintainability",
      locations: [{ file: "a.py", lines: { start: 1, end: 1 } }],
      evidence: [
        {
          kind: "external-analyzer",
          tool: "ruff",
          ruleId: "F401",
          version: "0.15.1",
          license: "MIT",
          url: undefined,
        },
      ],
    });
    expect(canSuppressInline(analyzer)).toBe(false);
  });

  it("writes the comment above the line, indented like it, in the file's own comment style", () => {
    const { entries } = realEntries(`function f() {\n    ${VAR_LINE}\n}\n`);
    const finding = byRule(entries, "no-var").latestFinding;
    expect(suppressionEdit(finding, `    ${VAR_LINE}`, "  legacy code  ")).toEqual({
      line: 1,
      text: "    // debuggatha-ignore-next-line no-var -- legacy code\n",
    });

    const python: Finding = {
      ...finding,
      locations: [{ file: "tool.py", lines: { start: 3, end: 3 }, columns: undefined } as never],
    };
    expect(suppressionEdit(python, "x = 1", "why").text).toBe(
      "# debuggatha-ignore-next-line no-var -- why\n",
    );
  });
});

describe("hoverMarkdown", () => {
  it("names the rule and pack, says why, and shows the fix with its before and after", () => {
    const { entries } = realEntries(`${VAR_LINE}\n`);
    const md = hoverMarkdown(byRule(entries, "no-var"));
    expect(md).toContain("**low**");
    expect(md).toContain("`no-var`");
    expect(md).toContain("confidence: high");
    expect(md).toContain("**Fix:**");
    expect(md).toContain("// before");
    expect(md).toContain("// after");
    expect(md).toContain("Suppress in code");
  });

  it("links to commands with the finding's id, encoded", () => {
    const { entries } = realEntries(`${VAR_LINE}\n`);
    const entry = byRule(entries, "no-var");
    const md = hoverMarkdown(entry);
    expect(md).toContain(commandLink("Resolve", "debuggatha.resolveFindingById", entry.id));
    expect(md).toContain(
      `command:debuggatha.dismissFindingById?${encodeURIComponent(JSON.stringify([entry.id]))}`,
    );
  });

  it("credits an analyzer's finding to the tool, with its version, license, and documentation", () => {
    const finding = createFinding({
      title: "`os` imported but unused",
      explanation: "Debuggatha ran Ruff and reports what it found; boilerplate.",
      severity: "medium",
      confidence: "high",
      category: "reliability",
      locations: [{ file: "a.py", lines: { start: 1, end: 1 } }],
      evidence: [
        {
          kind: "external-analyzer",
          tool: "ruff",
          ruleId: "F401",
          version: "0.15.1",
          license: "MIT",
          url: "https://docs.astral.sh/ruff/rules/unused-import",
        },
      ],
    });
    const md = hoverMarkdown({ id: "e1", latestFinding: finding, status: "open" } as LedgerEntry);
    expect(md).toContain(
      "`ruff:F401` · ruff 0.15.1, MIT · [documentation](https://docs.astral.sh/ruff/rules/unused-import)",
    );
    expect(md).not.toContain("boilerplate");
    expect(md).not.toContain("Suppress in code"); // a comment cannot silence a tool's finding
  });

  it("says a model's finding is a suspicion, and which model", () => {
    const finding = createFinding({
      title: "Off by one",
      explanation: "Reads one past the end.",
      severity: "medium",
      confidence: "low",
      category: "reliability",
      locations: [{ file: "a.ts", lines: { start: 3, end: 3 } }],
      evidence: [
        {
          kind: "semantic-review",
          role: "detection",
          provider: "ollama",
          model: "qwen2.5-coder:7b",
          verdict: undefined,
          reason: "r",
          topic: "off-by-one",
        },
      ],
    });
    expect(provenanceOf(finding)).toEqual({
      kind: "model",
      provider: "ollama",
      model: "qwen2.5-coder:7b",
    });
    const md = hoverMarkdown({ id: "e2", latestFinding: finding, status: "open" } as LedgerEntry);
    expect(md).toContain("a suspicion raised by a model (ollama qwen2.5-coder:7b)");
    expect(md).toContain("check it before acting");
    expect(md).toContain("confidence: low");
  });

  it("does not let a finding's text inject markup", () => {
    const finding = createFinding({
      title: "<script>alert(1)</script>",
      explanation: "<img src=x onerror=1>",
      severity: "low",
      confidence: "high",
      category: "maintainability",
      locations: [{ file: "a.ts", lines: { start: 1, end: 1 } }],
      evidence: [{ kind: "code", file: "a.ts", lines: undefined, excerpt: undefined }],
    });
    const md = hoverMarkdown({ id: "e3", latestFinding: finding, status: "open" } as LedgerEntry);
    expect(md).not.toContain("<script>");
    expect(md).not.toContain("<img");
    expect(md).toContain("&lt;script&gt;");
  });
});
