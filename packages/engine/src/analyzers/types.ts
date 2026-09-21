import type { Severity } from "@debuggatha/core";

/**
 * What running an analyzer can do to the machine, and so whether it may run
 * without being asked:
 *   safe               reads source files only (a formatter, a secret scanner)
 *   runs-project-code  loads code the repository controls: a lint config that is
 *                      JavaScript, a build script, a PHP bootstrap file
 *   network            sends data (package names and versions) to a service
 * Only `safe` analyzers run by default. The others need to be enabled by name,
 * by the person running Debuggatha: never by a tool argument or repository file.
 */
export type AnalyzerTrust = "safe" | "runs-project-code" | "network";

/** One finding in a common shape, whatever the tool's own format was. */
export interface AnalyzerFinding {
  ruleId: string;
  message: string;
  severity: Severity;
  category: string;
  /** Repository-relative, POSIX separators. */
  file: string;
  line?: number;
  column?: number;
  endLine?: number;
  endColumn?: number;
  url?: string;
  /** The line itself is a secret or holds one, so it must not be copied into a report. */
  sensitive?: boolean;
}

export interface ParseContext {
  rootDir: string;
}

export interface AnalyzerRunContext {
  rootDir: string;
  /** Repository-relative files to look at, or `undefined` for the whole repository. */
  files: readonly string[] | undefined;
}

export interface AnalyzerCommand {
  args: string[];
  /** Files to remove after the run (a temporary config, for instance). */
  cleanup?: () => void;
}

export interface AnalyzerAdapter {
  /** Stable id, used to enable it (`--analyzer ruff`) and to name it in evidence. */
  id: string;
  name: string;
  /** SPDX id of the tool's own license. Debuggatha runs the tool; it never bundles or redistributes it. */
  license: string;
  homepage: string;
  /** What the tool covers, in a few words. */
  covers: string;
  trust: AnalyzerTrust;
  /** Whether this repository uses what the tool analyzes; cheap, reads the file listing only. */
  applies(repo: RepoFacts): boolean;
  /** Where the executable may be: repository-local paths first, then names on `PATH`. */
  binaries: { local: string[]; names: string[] };
  /** Arguments placed before the tool's own (`cargo` needs `clippy`). */
  prefix?: string[];
  versionArgs: string[];
  command(context: AnalyzerRunContext): AnalyzerCommand;
  /** Exit codes that mean "ran, and may have found something". Anything else is a failure. */
  okExitCodes: readonly number[];
  parse(stdout: string, context: ParseContext): AnalyzerFinding[];
  /** Internal rule ids (same file and line) this tool's rule reports too; the tool's finding replaces ours. */
  equivalentTo?: Record<string, string>;
  /** Internal rule ids this tool answers for entirely, so ours would only repeat or contradict it. */
  supersedes?: string[];
  /** Regex a repository-relative file must match to be handed to the tool when a review has a scope. */
  fileFilter?: RegExp;
}

/** What the repository looks like, computed once per review and shared by every adapter's `applies`. */
export interface RepoFacts {
  rootDir: string;
  files: readonly string[];
  has(pattern: RegExp): boolean;
}

export type AnalyzerStatus = "ran" | "skipped" | "failed";

/** What happened to one analyzer in one review, reported whether or not it found anything. */
export interface AnalyzerRun {
  id: string;
  name: string;
  license: string;
  trust: AnalyzerTrust;
  status: AnalyzerStatus;
  /** Why it did not run or failed; absent when it ran. */
  reason?: string;
  version?: string;
  findings: number;
  durationMs: number;
}

export interface AnalyzerOptions {
  /** `off` runs none. `auto` (the default when options are given) runs the safe ones that are installed and apply. */
  mode?: "auto" | "off";
  /** Analyzers to run even though they load project code or use the network: the person's explicit choice. */
  enable?: readonly string[];
  /** Analyzers to leave out, by id (a repository's own choice; it can only narrow what runs). */
  disable?: readonly string[];
  /** Seconds one analyzer may run before it is stopped. */
  timeoutSeconds?: number;
  /** How the caller enables an analyzer (a flag, an environment variable), for the message that says why one did not run. */
  enableHint?: (id: string) => string;
}
