import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import type { WorkflowContextItem } from "../types.js";

const RELEASE_CONFIG_FILES = [
  ".release-please-manifest.json",
  "release-please-config.json",
  ".releaserc",
  ".releaserc.json",
  ".releaserc.yml",
  "CHANGELOG.md",
];

const COMMIT_CONVENTION_FILES = [
  "commitlint.config.js",
  "commitlint.config.ts",
  "commitlint.config.mjs",
  ".commitlintrc.json",
];

/**
 * Development Workflow (Epic 14). Deterministic, filename/marker-based
 * detection — no YAML parser dependency (same trade-off
 * `@debuggatha/repository-intelligence`'s README already documents for
 * `rustfmt.toml`/`.editorconfig`: a hand-rolled bounded extractor over a
 * full spec implementation). CI detection reads each workflow file's
 * `on:` trigger line via regex rather than parsing the whole YAML
 * document — enough to report *that* CI runs and on what triggers,
 * without a general YAML parser this repository has consistently avoided
 * pulling in for one or two config shapes.
 */
export function detectWorkflow(rootDir: string): WorkflowContextItem[] {
  const items: WorkflowContextItem[] = [];

  const workflowsDir = join(rootDir, ".github", "workflows");
  if (existsSync(workflowsDir)) {
    let files: string[] = [];
    try {
      files = readdirSync(workflowsDir).filter((f) => f.endsWith(".yml") || f.endsWith(".yaml"));
    } catch {
      files = [];
    }
    for (const file of files) {
      const relativePath = join(".github", "workflows", file);
      let content = "";
      try {
        content = readFileSync(join(workflowsDir, file), "utf8");
      } catch {
        continue;
      }
      const triggers = extractTriggers(content);
      items.push({
        id: `workflow:ci:${relativePath}`,
        category: "workflow",
        confidence: "detected",
        evidence: [
          {
            file: relativePath,
            detail: `GitHub Actions workflow present, triggers: ${triggers.join(", ") || "unknown"}`,
          },
        ],
        source: relativePath,
        workflowKind: "ci",
        detail: `CI workflow "${file}" triggers on: ${triggers.join(", ") || "unknown"}`,
      });
    }
  }

  const releaseFile = RELEASE_CONFIG_FILES.find((file) => existsSync(join(rootDir, file)));
  if (releaseFile) {
    items.push({
      id: "workflow:release",
      category: "workflow",
      confidence: "detected",
      evidence: [{ file: releaseFile, detail: `${releaseFile} present at repository root` }],
      source: releaseFile,
      workflowKind: "release",
      detail: `Release/versioning managed via ${releaseFile}`,
    });
  }

  const commitConventionFile = COMMIT_CONVENTION_FILES.find((file) =>
    existsSync(join(rootDir, file)),
  );
  if (commitConventionFile) {
    items.push({
      id: "workflow:commit-convention",
      category: "workflow",
      confidence: "detected",
      evidence: [
        {
          file: commitConventionFile,
          detail: `${commitConventionFile} present at repository root`,
        },
      ],
      source: commitConventionFile,
      workflowKind: "commit-convention",
      detail: "Commit messages are linted against a convention (commitlint).",
    });
  }

  const branchesDir = join(rootDir, ".git", "refs", "heads");
  if (existsSync(branchesDir)) {
    let branches: string[] = [];
    try {
      branches = readdirSync(branchesDir);
    } catch {
      branches = [];
    }
    // Git stores "feature/x" as a directory named "feature" containing "x" —
    // a top-level entry matching one of these names (directory or not) is
    // itself the signal, without needing to recurse into it.
    const hasReleaseBranches = branches.some((b) => /^(release|hotfix|feature)$/.test(b));
    if (hasReleaseBranches) {
      items.push({
        id: "workflow:branching-convention",
        category: "workflow",
        confidence: "detected",
        evidence: [
          { file: ".git/refs/heads", detail: `Local branches include: ${branches.join(", ")}` },
        ],
        source: ".git/refs/heads",
        workflowKind: "branching-convention",
        detail: "Branch names follow a release/hotfix/feature prefix convention.",
      });
    }
  }

  return items;
}

function extractTriggers(workflowContent: string): string[] {
  const match = /^on:\s*$/m.exec(workflowContent);
  if (!match) {
    const inline = /^on:\s*\[(.*)\]/m.exec(workflowContent);
    if (inline?.[1]) return inline[1].split(",").map((s) => s.trim().replace(/["']/g, ""));
    const singleLine = /^on:\s*([a-zA-Z_]+)\s*$/m.exec(workflowContent);
    if (singleLine?.[1]) return [singleLine[1]];
    return [];
  }
  const afterOn = workflowContent.slice(match.index + match[0].length);
  const triggers: string[] = [];
  const lines = afterOn.split("\n");
  for (const line of lines) {
    const keyMatch = /^\s{2}([a-zA-Z_]+):/.exec(line);
    if (keyMatch?.[1]) triggers.push(keyMatch[1]);
    else if (line.trim() && !/^\s/.test(line)) break;
  }
  return triggers;
}
