import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
  biome,
  clippy,
  detekt,
  eslint,
  gitleaks,
  ktlint,
  osvScanner,
  phpstan,
  ruff,
} from "./adapters.js";
import type { AnalyzerAdapter } from "./types.js";

/** Output recorded from the real tools (see `fixtures/`), paths rewritten to `/repo`. */
const fixture = (name: string) =>
  readFileSync(new URL(`./fixtures/${name}`, import.meta.url), "utf8");
const parse = (adapter: AnalyzerAdapter, name: string) =>
  adapter.parse(fixture(name), { rootDir: "/repo" });

describe("gitleaks", () => {
  const found = parse(gitleaks, "gitleaks.sarif");

  test("reports each secret as a critical security finding", () => {
    expect(found.map((item) => item.ruleId)).toEqual(["aws-access-token", "github-pat"]);
    expect(
      found.every((item) => item.severity === "critical" && item.category === "security"),
    ).toBe(true);
  });

  test("locates the secret and marks the line sensitive so it is never excerpted", () => {
    expect(found[0]).toMatchObject({ file: "config.env", line: 1, column: 19, endColumn: 38 });
    expect(found.every((item) => item.sensitive)).toBe(true);
  });
});

describe("biome", () => {
  const found = parse(biome, "biome-report.json");

  test("reads every diagnostic with its rule category", () => {
    expect(found.map((item) => item.ruleId)).toEqual([
      "lint/suspicious/noExplicitAny",
      "lint/style/useConst",
      "lint/correctness/noUnusedVariables",
      "lint/suspicious/noDoubleEquals",
      "lint/security/noGlobalEval",
    ]);
  });

  test("maps the rule group to a category and the level to a severity", () => {
    const byRule = Object.fromEntries(found.map((item) => [item.ruleId, item]));
    expect(byRule["lint/security/noGlobalEval"]?.category).toBe("security");
    expect(byRule["lint/suspicious/noDoubleEquals"]?.category).toBe("reliability");
    expect(byRule["lint/style/useConst"]?.category).toBe("maintainability");
    expect(byRule["lint/suspicious/noExplicitAny"]).toMatchObject({
      severity: "low",
      file: "t.ts",
      line: 3,
      column: 8,
      url: "https://biomejs.dev/linter/rules/no-explicit-any",
    });
  });
});

describe("eslint", () => {
  const found = parse(eslint, "eslint.json");

  test("uses the repository's severity: error is medium, warn is low", () => {
    const byRule = Object.fromEntries(found.map((item) => [item.ruleId, item]));
    expect(byRule.eqeqeq?.severity).toBe("medium");
    expect(byRule["no-var"]?.severity).toBe("low");
  });

  test("files eval under security and carries the rule's page", () => {
    const evalRule = found.find((item) => item.ruleId === "no-eval");
    expect(evalRule).toMatchObject({
      category: "security",
      file: "app.js",
      url: "https://eslint.org/docs/latest/rules/no-eval",
    });
  });

  test("keeps the end of the range", () => {
    expect(found.find((item) => item.ruleId === "no-var")).toMatchObject({
      line: 1,
      column: 1,
      endLine: 1,
      endColumn: 11,
    });
  });
});

describe("ruff", () => {
  const found = parse(ruff, "ruff.json");

  test("reads codes, positions, and documentation urls", () => {
    expect(found.map((item) => item.ruleId)).toEqual(["I001", "F401", "F841"]);
    expect(found[1]).toMatchObject({ file: "app.py", line: 1, column: 12 });
    expect(found[1]?.url).toContain("docs.astral.sh/ruff/rules");
  });

  test("Pyflakes findings are reliability issues; import order is only style", () => {
    expect(found.find((item) => item.ruleId === "F401")).toMatchObject({
      category: "reliability",
      severity: "medium",
    });
    expect(found.find((item) => item.ruleId === "I001")).toMatchObject({
      category: "maintainability",
      severity: "low",
    });
  });
});

describe("ktlint", () => {
  test("reads SARIF that follows a log line", () => {
    const found = parse(ktlint, "ktlint.stdout");
    expect(found).toHaveLength(3);
    expect(found[0]).toMatchObject({
      ruleId: "standard:no-wildcard-imports",
      file: "kt/Main.kt",
      line: 1,
      column: 1,
      severity: "low",
      category: "maintainability",
    });
  });
});

describe("detekt", () => {
  test("resolves file URIs against the repository and links the rule", () => {
    const found = parse(detekt, "detekt.sarif");
    expect(found).toHaveLength(2);
    expect(found[0]).toMatchObject({
      ruleId: "detekt.style.UnusedPrivateProperty",
      file: "kt/Main.kt",
      line: 3,
      column: 9,
      endColumn: 15,
      url: "https://detekt.dev/style.html#unusedprivateproperty",
    });
  });
});

describe("phpstan", () => {
  const found = parse(phpstan, "phpstan.json");

  test("turns every message into a finding under the identifier PHPStan gave it", () => {
    expect(found).toHaveLength(2);
    expect(found[0]).toMatchObject({
      ruleId: "property.notFound",
      file: "User.php",
      line: 4,
      category: "reliability",
      url: "https://phpstan.org/error-identifiers/property.notFound",
    });
  });

  test("a clean project reports `passed` with no files at all", () => {
    // What PHPStan printed for a real Laravel application at level 5.
    expect(
      phpstan.parse('{"tool":"phpstan","result":"passed","errors":0}', { rootDir: "/repo" }),
    ).toEqual([]);
    expect(() =>
      phpstan.parse('{"tool":"phpstan","result":"failed","errors":1}', { rootDir: "/repo" }),
    ).toThrow("failed run without saying why");
  });

  test("a run that crashed is a failure, not a clean report", () => {
    // Recorded from PHPStan on a real Laravel application that exhausted PHP's default memory.
    expect(() => parse(phpstan, "phpstan-crash.json")).toThrow(
      "PHPStan could not finish: Child process error: PHPStan process crashed because it reached configured PHP memory limit: 128M",
    );
  });
});

describe("clippy", () => {
  const found = parse(clippy, "clippy.jsonl");

  test("reads compiler messages that carry a lint code and ignores build noise", () => {
    expect(found.map((item) => item.ruleId).sort()).toEqual([
      "clippy::len_zero",
      "clippy::useless_vec",
    ]);
  });

  test("uses the primary span and Clippy's documentation link", () => {
    expect(found.find((item) => item.ruleId === "clippy::len_zero")).toMatchObject({
      file: "src/main.rs",
      line: 7,
      column: 8,
      severity: "low",
      url: expect.stringContaining("rust-clippy"),
    });
  });
});

describe("osv-scanner", () => {
  const found = parse(osvScanner, "osv-scanner.sarif");

  test("scores severity from the advisory, not from SARIF's level", () => {
    const byRule = new Map(found.map((item) => [item.ruleId, item]));
    expect(byRule.get("CVE-2021-23337")?.severity).toBe("high"); // 8.1
    expect(byRule.get("CVE-2020-8203")?.severity).toBe("high"); // 7.4
    expect(byRule.get("CVE-2020-28500")?.severity).toBe("medium"); // 5.3
  });

  test("points at the lockfile, which has no line", () => {
    expect(found[0]).toMatchObject({ file: "package-lock.json", category: "security" });
    expect(found[0]?.line).toBeUndefined();
    expect(found[0]?.message).toContain("lodash@4.17.15");
  });

  test("an empty report (nothing to scan) is no findings", () => {
    expect(osvScanner.parse("", { rootDir: "/repo" })).toEqual([]);
  });
});

test("a path outside the repository is not a finding of the repository", () => {
  const report = JSON.stringify([
    { code: "F401", message: "x", filename: "/elsewhere/a.py", location: { row: 1, column: 1 } },
  ]);
  expect(ruff.parse(report, { rootDir: "/repo" })).toEqual([]);
});
