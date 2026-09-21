import { afterEach, describe, expect, it } from "bun:test";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import type { Finding, Recommendation } from "./index.js";
import { listChangedFiles } from "./infrastructure/index.js";
import { executeReview } from "./pipeline.js";

let dirs: string[] = [];
afterEach(() => {
  for (const dir of dirs) rmSync(dir, { recursive: true, force: true });
  dirs = [];
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

function makeRepo(files: Record<string, string>, options: { commit?: boolean } = {}): string {
  const root = mkdtempSync(join(tmpdir(), "debuggatha-changes-"));
  dirs.push(root);
  write(root, files);
  git(root, "init", "-q", "-b", "main");
  if (options.commit !== false) {
    git(root, "add", "-A");
    git(root, "commit", "-q", "-m", "base");
  }
  return root;
}

const PKG = JSON.stringify({ name: "fixture", devDependencies: { typescript: "^5" } });

const ruleOf = (finding: Finding): string | undefined => {
  for (const evidence of finding.evidence) if ("ruleId" in evidence) return evidence.ruleId;
  return undefined;
};

describe("listChangedFiles", () => {
  it("names what was edited, added (tracked or not), and deleted", () => {
    const root = makeRepo({
      "package.json": PKG,
      ".gitignore": "scratch/\n",
      "src/a.ts": "export const a = 1;\n",
      "src/b.ts": "export const b = 1;\n",
      "src/c.ts": "export const c = 1;\n",
    });
    write(root, {
      "src/a.ts": "export const a = 2;\n",
      "src/d.ts": "export const d = 1;\n",
      "vendor/lib.js": "var v = 1;\n",
      "scratch/tmp.ts": "var t = 1;\n",
    });
    rmSync(join(root, "src", "b.ts"));

    const { changed, deleted, base } = listChangedFiles(root);
    expect(changed).toEqual(["src/a.ts", "src/d.ts"]);
    expect(deleted).toEqual(["src/b.ts"]);
    expect(base).toBe("HEAD");
  });

  it("measures against a ref when given one", () => {
    const root = makeRepo({ "package.json": PKG, "src/a.ts": "export const a = 1;\n" });
    write(root, { "src/a.ts": "export const a = 2;\n" });
    git(root, "commit", "-q", "-am", "edit");
    write(root, { "src/b.ts": "export const b = 1;\n" });
    git(root, "add", "-A");
    git(root, "commit", "-q", "-m", "add");

    expect(listChangedFiles(root, "HEAD~1").changed).toEqual(["src/b.ts"]);
    expect(listChangedFiles(root, "HEAD~2").changed).toEqual(["src/a.ts", "src/b.ts"]);
  });

  it("treats everything as new in a repository with no commits", () => {
    const root = makeRepo({ "package.json": PKG, "src/a.ts": "let a;\n" }, { commit: false });
    const { changed, base } = listChangedFiles(root);
    expect(base).toBeNull();
    expect(changed).toEqual(["package.json", "src/a.ts"]);
  });

  it("reports paths relative to the directory it is asked about", () => {
    const root = makeRepo({ "package.json": PKG, "packages/app/src/a.ts": "let a;\n" });
    write(root, { "packages/app/src/b.ts": "let b;\n", "top.ts": "let t;\n" });
    expect(listChangedFiles(join(root, "packages", "app")).changed).toEqual(["src/b.ts"]);
  });

  it("does not call a restored file deleted", () => {
    const root = makeRepo({ "package.json": PKG, "src/a.ts": "let a;\n" });
    rmSync(join(root, "src", "a.ts"));
    write(root, { "src/a.ts": "let a;\n" });
    expect(listChangedFiles(root).deleted).toEqual([]);
  });

  it("explains itself outside a git repository, and refuses an unsafe ref", () => {
    const plain = mkdtempSync(join(tmpdir(), "debuggatha-nogit-"));
    dirs.push(plain);
    expect(() => listChangedFiles(plain)).toThrow("not a git repository");
    const root = makeRepo({ "package.json": PKG });
    expect(() => listChangedFiles(root, "HEAD; touch pwned")).toThrow("Unsafe git ref");
  });
});

describe("the review loop, end to end in the engine", () => {
  const review = (root: string, changed: string[], deleted: string[] = []) =>
    executeReview({
      rootDir: root,
      scope: { kind: "files", paths: [...changed, ...deleted] },
      depth: "full",
      packIds: [],
      policyId: undefined,
      sourceName: "test",
    });

  it("gives findings an identity that survives reviews, and reports what changed between them", () => {
    const root = makeRepo({ "package.json": PKG, "src/ok.ts": "export const ok = 1;\n" });
    write(root, { "src/new.ts": "var a = 1;\nvar b = run(eval(x));\n" });

    const first = review(root, ["src/new.ts"]);
    expect(first.changes.introduced.map((ref) => ref.ruleId).sort()).toEqual(["no-eval", "no-var"]);
    expect(first.changes.fixed).toEqual([]);
    const ids = new Set(Object.values(first.ledgerIds));
    expect(ids.size).toBe(2);

    // Fix `eval`; leave `var`.
    write(root, { "src/new.ts": "var a = 1;\nvar b = run(JSON.parse(x));\n" });
    const second = review(root, ["src/new.ts"]);
    expect(second.changes.introduced).toEqual([]);
    expect(second.changes.fixed.map((ref) => ref.ruleId)).toEqual(["no-eval"]);
    expect(new Set(Object.values(second.ledgerIds))).toEqual(
      new Set([...ids].filter((id) => second.changes.fixed.every((ref) => ref.ledgerId !== id))),
    );
  });

  it("resolves what a deleted file held", () => {
    const root = makeRepo({ "package.json": PKG, "src/old.ts": "var legacy = 1;\n" });
    review(root, ["src/old.ts"]);
    rmSync(join(root, "src", "old.ts"));

    const after = review(root, [], ["src/old.ts"]);
    expect(after.result.findings).toEqual([]);
    expect(after.changes.fixed.map((ref) => ref.file)).toEqual(["src/old.ts"]);
  });

  it("brings back a finding that was closed and reappears", () => {
    const root = makeRepo({ "package.json": PKG, "src/a.ts": "var x = 1;\n" });
    review(root, ["src/a.ts"]);
    write(root, { "src/a.ts": "let x = 1;\n" });
    expect(review(root, ["src/a.ts"]).changes.fixed).toHaveLength(1);
    write(root, { "src/a.ts": "var x = 1;\n" });
    expect(review(root, ["src/a.ts"]).changes.reopened.map((ref) => ref.ruleId)).toEqual([
      "no-var",
    ]);
  });

  it("reads a path relative to the repository, not to the process", () => {
    const root = makeRepo({ "package.json": PKG, "src/a.ts": "var x = 1;\n" });
    expect(review(root, ["src/a.ts"]).result.findings).toHaveLength(1);
  });
});

describe("mechanical fixes: applying the suggested edit really fixes the finding", () => {
  function applyEdit(source: string, edit: NonNullable<Recommendation["edit"]>): string {
    const lines = source.split("\n");
    const line = lines[edit.line - 1] ?? "";
    lines[edit.line - 1] =
      line.slice(0, edit.startColumn - 1) + edit.replacement + line.slice(edit.endColumn - 1);
    return lines.join("\n");
  }

  const cases: {
    rule: string;
    file: string;
    source: string;
    fixed: string;
    safety: "safe" | "review";
  }[] = [
    {
      rule: "no-var",
      file: "src/a.ts",
      source: "  var total = 0;\n",
      fixed: "  let total = 0;\n",
      safety: "safe",
    },
    {
      rule: "eqeqeq",
      file: "src/a.ts",
      source: "if (a == b) {}\n",
      fixed: "if (a === b) {}\n",
      safety: "review",
    },
    {
      rule: "eqeqeq",
      file: "src/a.ts",
      source: "if (a != b) {}\n",
      fixed: "if (a !== b) {}\n",
      safety: "review",
    },
    {
      rule: "no-explicit-any",
      file: "src/a.ts",
      source: "function f(x: any) {}\n",
      fixed: "function f(x: unknown) {}\n",
      safety: "review",
    },
    {
      rule: "no-explicit-any",
      file: "src/a.ts",
      source: "const v = w as any;\n",
      fixed: "const v = w as unknown;\n",
      safety: "review",
    },
    {
      rule: "avoid-print",
      file: "lib/a.dart",
      source: "print('hi');\n",
      fixed: "debugPrint('hi');\n",
      safety: "safe",
    },
  ];

  for (const { rule, file, source, fixed, safety } of cases) {
    it(`${rule}: ${source.trim()} -> ${fixed.trim()}`, () => {
      const stack = file.endsWith(".dart")
        ? { "pubspec.yaml": "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n" }
        : { "package.json": PKG };
      const root = makeRepo({ ...stack, [file]: source });
      const findings = executeReview({
        rootDir: root,
        scope: { kind: "files", paths: [file] },
        depth: "full",
        packIds: [],
        policyId: undefined,
        sourceName: "test",
        persist: false,
      }).result.findings.filter((finding) => ruleOf(finding) === rule);
      const edit = findings[0]?.recommendations[0]?.edit;
      expect(edit).toBeDefined();
      expect(edit?.safety).toBe(safety);

      const patched = applyEdit(source, edit as NonNullable<typeof edit>);
      expect(patched).toBe(fixed);
      writeFileSync(join(root, file), patched);
      const again = executeReview({
        rootDir: root,
        scope: { kind: "files", paths: [file] },
        depth: "full",
        packIds: [],
        policyId: undefined,
        sourceName: "test",
        persist: false,
      }).result.findings.filter((finding) => ruleOf(finding) === rule);
      expect(again).toEqual([]);
      expect(readFileSync(join(root, file), "utf8")).toBe(fixed);
    });
  }

  it("offers no exact edit where the fix needs judgment", () => {
    const root = makeRepo({ "package.json": PKG, "src/a.ts": "run(eval(x));\n" });
    const [finding] = executeReview({
      rootDir: root,
      scope: { kind: "files", paths: ["src/a.ts"] },
      depth: "full",
      packIds: [],
      policyId: undefined,
      sourceName: "test",
      persist: false,
    }).result.findings;
    expect(finding?.recommendations[0]?.edit).toBeUndefined();
    expect(finding?.recommendations[0]?.example).toBeDefined();
  });
});
