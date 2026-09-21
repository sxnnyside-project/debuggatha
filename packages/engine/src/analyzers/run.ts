import { spawnSync } from "node:child_process";
import { accessSync, constants, existsSync, readFileSync, statSync } from "node:fs";
import { delimiter, isAbsolute, join } from "node:path";
import { createFinding, type Finding } from "@debuggatha/core";
import { isIgnoredPath, listScanFiles } from "../scan-scope/index.js";
import { ADAPTERS } from "./adapters.js";
import type {
  AnalyzerAdapter,
  AnalyzerFinding,
  AnalyzerOptions,
  AnalyzerRun,
  AnalyzerTrust,
  RepoFacts,
} from "./types.js";

const DEFAULT_TIMEOUT_SECONDS = 300;

const TRUST_REASON: Record<Exclude<AnalyzerTrust, "safe">, string> = {
  "runs-project-code": "it runs code the repository controls",
  network: "it sends package names and versions to a service",
};

function isExecutable(path: string): boolean {
  try {
    if (!statSync(path).isFile()) return false;
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

/** A repository-local install wins over one on `PATH`: it is the version the project pinned. */
export function findExecutable(rootDir: string, adapter: AnalyzerAdapter): string | undefined {
  for (const local of adapter.binaries.local) {
    const path = join(rootDir, local);
    if (isExecutable(path)) return path;
  }
  const directories = (process.env.PATH ?? "").split(delimiter).filter(Boolean);
  for (const name of adapter.binaries.names) {
    for (const directory of directories) {
      const path = join(directory, name);
      if (isExecutable(path)) return path;
    }
  }
  return undefined;
}

function readVersion(
  executable: string,
  adapter: AnalyzerAdapter,
  rootDir: string,
): string | undefined {
  const result = spawnSync(executable, [...(adapter.prefix ?? []), ...adapter.versionArgs], {
    cwd: rootDir,
    encoding: "utf8",
    timeout: 30_000,
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.error || result.status !== 0) return undefined;
  return /\d+\.\d+(?:\.\d+)?(?:[-+.\w]*)/.exec(`${result.stdout}\n${result.stderr}`)?.[0];
}

export function repoFacts(rootDir: string): RepoFacts {
  const files = listScanFiles(rootDir, /./);
  return { rootDir, files, has: (pattern) => files.some((file) => pattern.test(file)) };
}

export interface AnalyzerInventoryEntry {
  id: string;
  name: string;
  license: string;
  homepage: string;
  covers: string;
  trust: AnalyzerTrust;
  applies: boolean;
  installed: boolean;
  version: string | undefined;
  /** Whether a review would run it with the options given. */
  willRun: boolean;
  /** What stops it from running, when something does. */
  reason: string | undefined;
}

function gate(
  adapter: AnalyzerAdapter,
  options: AnalyzerOptions,
  applies: boolean,
  installed: boolean,
): string | undefined {
  if (options.mode === "off") return "analyzers are turned off";
  if (options.disable?.includes(adapter.id)) return "disabled in the repository's configuration";
  if (!applies) return "this repository has nothing it analyzes";
  if (!installed) return "not installed";
  if (adapter.trust !== "safe" && !options.enable?.includes(adapter.id)) {
    const how = options.enableHint?.(adapter.id) ?? `enable "${adapter.id}" explicitly`;
    return `${TRUST_REASON[adapter.trust]}; ${how}`;
  }
  return undefined;
}

/** What is installed and what a review would run: the answer behind `debuggatha analyzers`. */
export function inventoryAnalyzers(
  rootDir: string,
  options: AnalyzerOptions = {},
  adapters: readonly AnalyzerAdapter[] = ADAPTERS,
): AnalyzerInventoryEntry[] {
  const facts = repoFacts(rootDir);
  return adapters.map((adapter) => {
    const executable = findExecutable(rootDir, adapter);
    const applies = adapter.applies(facts);
    const reason = gate(adapter, options, applies, executable !== undefined);
    return {
      id: adapter.id,
      name: adapter.name,
      license: adapter.license,
      homepage: adapter.homepage,
      covers: adapter.covers,
      trust: adapter.trust,
      applies,
      installed: executable !== undefined,
      version: executable ? readVersion(executable, adapter, rootDir) : undefined,
      willRun: reason === undefined,
      reason,
    };
  });
}

export interface AnalyzerFindings {
  adapter: AnalyzerAdapter;
  version: string | undefined;
  findings: AnalyzerFinding[];
}

export interface AnalyzerRunResult {
  runs: AnalyzerRun[];
  results: AnalyzerFindings[];
}

/**
 * Runs each analyzer the options allow, as a separate process with no shell,
 * in the repository directory. A tool that is missing, unwanted, or that fails
 * never fails the review: it is reported in `runs` and the review goes on
 * without it.
 */
export function runAnalyzers(input: {
  rootDir: string;
  /** Repository-relative files under review, or `undefined` for the whole repository. */
  files: readonly string[] | undefined;
  options: AnalyzerOptions;
  adapters?: readonly AnalyzerAdapter[];
}): AnalyzerRunResult {
  const { rootDir, files, options } = input;
  const adapters = input.adapters ?? ADAPTERS;
  const runs: AnalyzerRun[] = [];
  const results: AnalyzerFindings[] = [];
  if (options.mode === "off") return { runs, results };

  const facts = repoFacts(rootDir);
  const inScope = files ? new Set(files) : undefined;
  const timeout = (options.timeoutSeconds ?? DEFAULT_TIMEOUT_SECONDS) * 1000;

  for (const adapter of adapters) {
    const base = {
      id: adapter.id,
      name: adapter.name,
      license: adapter.license,
      trust: adapter.trust,
      findings: 0,
      durationMs: 0,
    };
    const executable = findExecutable(rootDir, adapter);
    const blocked = gate(adapter, options, adapter.applies(facts), executable !== undefined);
    if (blocked !== undefined || executable === undefined) {
      runs.push({ ...base, status: "skipped", reason: blocked ?? "not installed" });
      continue;
    }

    // Tools that take paths get only the files under review; the others run whole and are filtered below.
    let targets: string[] | undefined;
    if (files && adapter.fileFilter) {
      const filter = adapter.fileFilter;
      targets = files
        // A file the review names may be gone (a deletion): there is nothing left to analyze.
        .filter((file) => filter.test(file) && existsSync(join(rootDir, file)))
        .map((file) => (file.startsWith("-") ? `./${file}` : file));
      if (targets.length === 0) {
        runs.push({ ...base, status: "skipped", reason: "no matching files in this review" });
        continue;
      }
    }

    const started = Date.now();
    const command = adapter.command({ rootDir, files: targets });
    try {
      const version = readVersion(executable, adapter, rootDir);
      const spawned = spawnSync(executable, [...(adapter.prefix ?? []), ...command.args], {
        cwd: rootDir,
        encoding: "utf8",
        timeout,
        maxBuffer: 512 * 1024 * 1024,
        stdio: ["ignore", "pipe", "pipe"],
      });
      const durationMs = Date.now() - started;

      if (spawned.error) {
        const timedOut = (spawned.error as NodeJS.ErrnoException).code === "ETIMEDOUT";
        runs.push({
          ...base,
          durationMs,
          status: "failed",
          reason: timedOut
            ? `timed out after ${timeout / 1000}s`
            : `could not start: ${spawned.error.message}`,
        });
        continue;
      }
      if (spawned.status === null || !adapter.okExitCodes.includes(spawned.status)) {
        const detail = (spawned.stderr || spawned.stdout).trim().split("\n").find(Boolean) ?? "";
        runs.push({
          ...base,
          durationMs,
          status: "failed",
          reason: `exited with code ${spawned.status ?? "?"}${detail ? `: ${detail.slice(0, 200)}` : ""}`,
        });
        continue;
      }

      let parsed: AnalyzerFinding[];
      try {
        parsed = adapter.parse(spawned.stdout, { rootDir });
      } catch (error) {
        runs.push({
          ...base,
          durationMs,
          status: "failed",
          reason: `output not understood: ${error instanceof Error ? error.message : String(error)}`,
        });
        continue;
      }

      const kept = parsed.filter(
        (finding) =>
          !isIgnoredPath(rootDir, finding.file) && (!inScope || inScope.has(finding.file)),
      );
      runs.push({
        ...base,
        durationMs,
        status: "ran",
        findings: kept.length,
        ...(version ? { version } : {}),
      });
      results.push({ adapter, version, findings: kept });
    } finally {
      command.cleanup?.();
    }
  }
  return { runs, results };
}

const EXCERPT_LIMIT = 200;

/** Reads lines of the reviewed files once, so an excerpt costs no more than one read per file. */
export class SourceLines {
  private readonly cache = new Map<string, string[] | undefined>();
  constructor(private readonly rootDir: string) {}

  line(file: string, line: number | undefined): string | undefined {
    if (line === undefined || isAbsolute(file)) return undefined;
    if (!this.cache.has(file)) {
      try {
        this.cache.set(file, readFileSync(join(this.rootDir, file), "utf8").split("\n"));
      } catch {
        this.cache.set(file, undefined);
      }
    }
    return this.cache.get(file)?.[line - 1]?.trim().slice(0, EXCERPT_LIMIT);
  }
}

/** One analyzer finding as a Debuggatha finding, carrying the tool, its license, and its rule as evidence. */
export function toFinding(
  found: AnalyzerFinding,
  source: { adapter: AnalyzerAdapter; version: string | undefined },
  lines: SourceLines,
  policyId: string,
): Finding {
  const { adapter, version } = source;
  const excerpt = found.sensitive
    ? undefined
    : (lines.line(found.file, found.line) ??
      (found.line === undefined ? found.message : undefined));
  const range =
    found.line !== undefined ? { start: found.line, end: found.endLine ?? found.line } : undefined;
  // A column range only means something on one line; across lines the end column belongs to another line.
  const oneLine = (found.endLine ?? found.line) === found.line;
  const columns =
    found.column !== undefined
      ? { start: found.column, end: oneLine ? (found.endColumn ?? found.column) : found.column }
      : undefined;

  return createFinding({
    title: found.message,
    explanation: `${adapter.name} rule ${found.ruleId}${found.url ? ` (${found.url})` : ""}. Debuggatha ran ${adapter.name} and reports what it found; the repository's own configuration decides which rules apply.`,
    severity: found.severity,
    confidence: "high",
    category: found.category,
    locations: [{ file: found.file, lines: range, ...(columns ? { columns } : {}) }],
    evidence: [
      { kind: "code", file: found.file, lines: range, excerpt },
      {
        kind: "external-analyzer",
        tool: adapter.id,
        ruleId: found.ruleId,
        version,
        license: adapter.license,
        url: found.url,
      },
    ],
    appliedPolicy: { id: policyId },
    recommendations: [
      {
        id: `${adapter.id}-${found.ruleId}-${found.file}-${found.line ?? 0}`,
        action: "investigate",
        summary: `Resolve ${adapter.name} ${found.ruleId}: ${found.message}`,
        rationale: found.url ? `See ${found.url}` : undefined,
        targetFile: found.file,
        targetLines: range,
      },
    ],
  });
}
