import { afterAll, describe, expect, it } from "bun:test";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import {
  getDefaultEnvironment,
  StdioClientTransport,
} from "@modelcontextprotocol/sdk/client/stdio.js";

/**
 * The acceptance test for the agent loop: a scripted agent talks to the real MCP server,
 * writes code with problems, reviews it, fixes what the review says using only what the
 * tools return, reviews again, and checks the ledger closed what was fixed.
 *
 * What is real: the server process, the protocol, the git repository, the detectors, the
 * ledger. What is simulated: the judgment an LLM would apply to a fix that is not mechanical;
 * `FIXERS` stands in for it, one small edit per rule, and is the only thing the agent knows
 * beyond the tool output.
 */

const BIN = resolve(import.meta.dir, "bin.ts");
const cleanup: string[] = [];
afterAll(() => {
  for (const dir of cleanup) rmSync(dir, { recursive: true, force: true });
});

const git = (cwd: string, ...args: string[]) =>
  execFileSync(
    "git",
    ["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", ...args],
    { cwd, stdio: "ignore" },
  );

function write(root: string, files: Record<string, string>) {
  for (const [path, content] of Object.entries(files)) {
    const full = join(root, path);
    mkdirSync(dirname(full), { recursive: true });
    writeFileSync(full, content);
  }
}

/** A committed, clean TypeScript + React project: what the agent starts from. */
function cleanRepo(): string {
  const root = mkdtempSync(join(tmpdir(), "debuggatha-agent-"));
  cleanup.push(root);
  write(root, {
    "package.json": JSON.stringify({
      name: "app",
      dependencies: { react: "^18.0.0" },
      devDependencies: { typescript: "^5.0.0" },
    }),
    "src/ok.ts": "export const ok = 1;\n",
  });
  git(root, "init", "-q", "-b", "main");
  git(root, "add", "-A");
  git(root, "commit", "-q", "-m", "base");
  return root;
}

async function connect(root: string): Promise<Client> {
  const client = new Client({ name: "scripted-agent", version: "0" });
  await client.connect(
    new StdioClientTransport({
      command: process.execPath,
      args: [BIN],
      env: {
        ...getDefaultEnvironment(),
        DEBUGGATHA_LOG_LEVEL: "error",
        DEBUGGATHA_REPOSITORY_ROOT: root,
      },
    }),
  );
  return client;
}

interface Finding {
  ledgerId?: string;
  locations: { file: string; lines: { start: number }; columns: { start: number } }[];
  recommendations: {
    summary: string;
    example?: { before: string; after: string };
    edit?: {
      line: number;
      startColumn: number;
      endColumn: number;
      replacement: string;
      safety: string;
    };
  }[];
  evidence: { ruleId?: string }[];
}
interface Ref {
  ledgerId?: string;
  ruleId?: string;
  file: string;
}
interface Review {
  findings: Finding[];
  changes: { introduced: Ref[]; fixed: Ref[]; reopened: Ref[] };
  changedFiles: string[];
  deletedFiles: string[];
  base: string | null;
  note?: string;
  suppressedFindings: unknown[];
}

const call = async <T>(client: Client, name: string, args: Record<string, unknown> = {}) => {
  const result = await client.callTool({ name, arguments: args });
  return result.structuredContent as T;
};
const ruleOf = (finding: Finding) => finding.evidence.find((e) => e.ruleId)?.ruleId ?? "";

/** The judgment an LLM would bring where the fix is not mechanical: one small rewrite per rule. */
const FIXERS: Record<string, (line: string) => string> = {
  "no-eval": (line) => line.replace("eval(", "JSON.parse("),
  "no-implied-eval": (line) => line.replace(/new Function\(.*\)/, "((a: number) => a)"),
  "prefer-template-literals": (line) => line.replace('"Hello, " + name + "!"', "`Hello, ${name}!`"),
  "no-extend-native": () => "export const last = <T>(items: T[]) => items[items.length - 1];",
  "no-hardcoded-secrets": (line) => line.replace(/"sk_live_[^"]+"/, "process.env.API_KEY"),
  "no-dangerously-set-inner-html": (line) =>
    line.replace(/<div dangerouslySetInnerHTML=\{\{ __html: html \}\} \/>/, "<div>{html}</div>"),
};

/** Fixes findings using only what the review returned; returns how many it could act on. */
function fix(root: string, findings: Finding[]): number {
  const files = new Map<string, string[]>();
  const linesOf = (file: string) => {
    if (!files.has(file)) files.set(file, readFileSync(join(root, file), "utf8").split("\n"));
    return files.get(file) as string[];
  };

  // Bottom to top, right to left, so an edit never moves the columns of one still to come.
  const ordered = [...findings].sort(
    (a, b) =>
      (b.locations[0]?.lines.start ?? 0) - (a.locations[0]?.lines.start ?? 0) ||
      (b.locations[0]?.columns.start ?? 0) - (a.locations[0]?.columns.start ?? 0),
  );

  let acted = 0;
  for (const finding of ordered) {
    const file = finding.locations[0]?.file as string;
    const lines = linesOf(file);
    const edit = finding.recommendations[0]?.edit;
    if (edit) {
      const line = lines[edit.line - 1] ?? "";
      lines[edit.line - 1] =
        line.slice(0, edit.startColumn - 1) + edit.replacement + line.slice(edit.endColumn - 1);
      acted += 1;
      continue;
    }
    const fixer = FIXERS[ruleOf(finding)];
    if (!fixer) continue;
    const index = (finding.locations[0]?.lines.start ?? 1) - 1;
    lines[index] = fixer(lines[index] ?? "");
    acted += 1;
  }
  for (const [file, lines] of files) writeFileSync(join(root, file), lines.join("\n"));
  return acted;
}

const AGENT_CODE = {
  "src/users.ts": [
    "var first = 1;",
    "var second = 2;",
    "export function same(a: number, b: number) { return a == b; }",
    "export function load(input: any) { return input; }",
    "export const total = first + second;",
    "",
  ].join("\n"),
  "src/run.ts": [
    "export const parsed = eval(source);",
    'export const make = new Function("a", "return a");',
    'export const greeting = "Hello, " + name + "!";',
    "Array.prototype.last = function () { return this[this.length - 1]; };",
    "",
  ].join("\n"),
  "src/pay.ts": 'export const apiKey = "sk_live_1234567890abcdef";\n',
  "src/view.tsx":
    "export const View = ({ html }: { html: string }) => <div dangerouslySetInnerHTML={{ __html: html }} />;\n",
};

describe("agent loop: write, review, fix, verify", () => {
  it("fixes at least 80% of what a change introduces within two rounds, and the ledger closes them", async () => {
    const root = cleanRepo();
    write(root, AGENT_CODE); // new files, untracked: a plain git diff would not see them
    const client = await connect(root);
    try {
      const first = await call<Review>(client, "review_changes");
      expect(first.changedFiles.sort()).toEqual(Object.keys(AGENT_CODE).sort());
      expect(first.findings.length).toBeGreaterThanOrEqual(10);
      const introduced = first.changes.introduced;
      expect(introduced.length).toBeGreaterThanOrEqual(9);
      // Every finding has an identity the next call can use.
      expect(first.findings.every((f) => f.ledgerId)).toBe(true);
      expect(introduced.every((ref) => ref.ledgerId)).toBe(true);

      // Round one: fix.
      const acted = fix(root, first.findings);
      expect(acted).toBe(first.findings.length);

      // Round two: verify.
      const second = await call<Review>(client, "review_changes");
      expect(second.changes.introduced).toEqual([]);
      const fixedIds = new Set(second.changes.fixed.map((ref) => ref.ledgerId));
      const introducedIds = new Set(introduced.map((ref) => ref.ledgerId));
      const closed = [...introducedIds].filter((id) => fixedIds.has(id));
      expect(closed.length / introducedIds.size).toBeGreaterThanOrEqual(0.8);
      // The ids the agent held from round one are the ones the ledger closed.
      expect([...fixedIds].every((id) => introducedIds.has(id))).toBe(true);
      expect(second.findings).toEqual([]);

      const resolved = await call<{ total: number }>(client, "list_findings", {
        status: ["resolved"],
      });
      expect(resolved.total).toBe(closed.length);
      const open = await call<{ total: number }>(client, "list_findings", { status: ["open"] });
      expect(open.total).toBe(0);
    } finally {
      await client.close();
    }
  });

  it("does not pass a fix that breaks something else", async () => {
    const root = cleanRepo();
    write(root, { "src/a.ts": "var count = 0;\n" });
    const client = await connect(root);
    try {
      const first = await call<Review>(client, "review_changes");
      expect(first.changes.introduced).toHaveLength(1);
      // A careless fix: trades one problem for another.
      write(root, { "src/a.ts": "let count: any = 0;\n" });
      const second = await call<Review>(client, "review_changes");
      expect(second.changes.fixed.map((ref) => ref.ruleId)).toEqual(["no-var"]);
      expect(second.changes.introduced.map((ref) => ref.ruleId)).toEqual(["no-explicit-any"]);
    } finally {
      await client.close();
    }
  });

  it("notices that deleting a file resolves what it held", async () => {
    const root = cleanRepo();
    write(root, { "src/temp.ts": "var scratch = 1;\n" });
    const client = await connect(root);
    try {
      expect((await call<Review>(client, "review_changes")).changes.introduced).toHaveLength(1);
      rmSync(join(root, "src", "temp.ts"));
      const after = await call<Review>(client, "review_changes");
      expect(after.changedFiles).toEqual([]);
      expect(after.changes.fixed.map((ref) => ref.file)).toEqual(["src/temp.ts"]);
    } finally {
      await client.close();
    }
  });

  it("measures against a ref when asked", async () => {
    const root = cleanRepo();
    write(root, { "src/a.ts": "var a = 1;\n" });
    git(root, "add", "-A");
    git(root, "commit", "-q", "-m", "agent work");
    const client = await connect(root);
    try {
      const sinceLast = await call<Review>(client, "review_changes");
      expect(sinceLast.changedFiles).toEqual([]);
      expect(sinceLast.findings).toEqual([]);
      expect(sinceLast.note).toContain("Nothing changed since HEAD");
      const sinceBase = await call<Review>(client, "review_changes", { base: "HEAD~1" });
      expect(sinceBase.changedFiles).toEqual(["src/a.ts"]);
      expect(sinceBase.base).toBe("HEAD~1");
      expect(sinceBase.changes.introduced).toHaveLength(1);
    } finally {
      await client.close();
    }
  });

  it("explains itself outside a git repository and refuses an unsafe ref", async () => {
    const plain = mkdtempSync(join(tmpdir(), "debuggatha-nogit-"));
    cleanup.push(plain);
    const client = await connect(plain);
    try {
      const result = await client.callTool({ name: "review_changes", arguments: {} });
      expect(result.isError).toBe(true);
      expect(JSON.stringify(result.content)).toContain("not a git repository");
    } finally {
      await client.close();
    }

    const root = cleanRepo();
    const other = await connect(root);
    try {
      const result = await other.callTool({
        name: "review_changes",
        arguments: { base: "HEAD; touch pwned" },
      });
      expect(result.isError).toBe(true);
      expect(existsSync(join(root, "pwned"))).toBe(false);
    } finally {
      await other.close();
    }
  });
});

describe("agent loop: understanding and accepting a finding", () => {
  it("explains a finding well enough to act on it", async () => {
    const root = cleanRepo();
    write(root, { "src/a.ts": "  var total = 0;\n" });
    const client = await connect(root);
    try {
      const [finding] = (await call<Review>(client, "review_changes")).findings;
      const explained = await call<{
        finding: {
          ledgerId: string;
          line: number;
          column: number;
          status: string;
          excerpt: string;
        };
        rule: { id: string };
        why: string;
        fix: string;
        example: { before: string; after: string };
        edit: { replacement: string; safety: string };
        ifYouAcceptIt: { inlineComment: string; durable: string };
        history: { state: string }[];
      }>(client, "explain_finding", { findingId: finding?.ledgerId });

      expect(explained.finding).toMatchObject({ line: 1, column: 3, status: "open" });
      expect(explained.finding.excerpt).toBe("var total = 0;");
      expect(explained.rule.id).toBe("no-var");
      expect(explained.why).toContain("hoisted");
      expect(explained.fix).toContain("const");
      expect(explained.example.after).toBe("let total = 0;");
      expect(explained.edit).toMatchObject({ replacement: "let", safety: "safe" });
      expect(explained.ifYouAcceptIt.inlineComment).toBe(
        "// debuggatha-ignore-next-line no-var -- <why this is acceptable>",
      );
      expect(explained.ifYouAcceptIt.durable).toContain("suppress_finding");
      expect(explained.history.map((h) => h.state)).toEqual(["open"]);
    } finally {
      await client.close();
    }
  });

  it("accepts a finding only with a real reason, and remembers why", async () => {
    const root = cleanRepo();
    write(root, { "src/a.ts": "export const run = (code: string) => eval(code);\n" });
    const client = await connect(root);
    try {
      const [finding] = (await call<Review>(client, "review_changes")).findings;
      const findingId = finding?.ledgerId as string;

      const tooShort = await client.callTool({
        name: "suppress_finding",
        arguments: { findingId, reason: "ok" },
      });
      expect(tooShort.isError).toBeTruthy();

      const reason = "sandboxed plugin loader; input never leaves the vm";
      const accepted = await call<{ memoryItemId: string; ruleId: string; effect: string }>(
        client,
        "suppress_finding",
        { findingId, reason },
      );
      expect(accepted.ruleId).toBe("no-eval");
      expect(accepted.effect).toContain("memory.json");

      const memory = JSON.parse(readFileSync(join(root, ".debuggatha", "memory.json"), "utf8"));
      const item = memory.items.find((i: { id: string }) => i.id === accepted.memoryItemId);
      expect(item).toMatchObject({ category: "suppression", status: "active", rationale: reason });

      const after = await call<Review>(client, "review_changes");
      expect(after.findings).toEqual([]);
      expect(after.suppressedFindings).toHaveLength(1);
      const dismissed = await call<{ total: number }>(client, "list_findings", {
        status: ["dismissed"],
      });
      expect(dismissed.total).toBe(1);
    } finally {
      await client.close();
    }
  });

  it("honors the inline comment explain_finding suggests", async () => {
    const root = cleanRepo();
    write(root, { "src/a.ts": "export const run = (code: string) => eval(code);\n" });
    const client = await connect(root);
    try {
      const [finding] = (await call<Review>(client, "review_changes")).findings;
      const { ifYouAcceptIt } = await call<{ ifYouAcceptIt: { inlineComment: string } }>(
        client,
        "explain_finding",
        { findingId: finding?.ledgerId },
      );
      const comment = ifYouAcceptIt.inlineComment.replace(
        "<why this is acceptable>",
        "sandboxed plugin loader",
      );
      write(root, {
        "src/a.ts": `${comment}\nexport const run = (code: string) => eval(code);\n`,
      });
      const after = await call<Review & { suppressedInline: { reason: string }[] }>(
        client,
        "review_changes",
      );
      expect(after.findings).toEqual([]);
      expect(after.suppressedInline.map((s) => s.reason)).toEqual(["sandboxed plugin loader"]);
    } finally {
      await client.close();
    }
  });
});
