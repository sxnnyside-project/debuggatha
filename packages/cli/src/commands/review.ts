import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  addedLinesByFile,
  createLocalProvider,
  executeReview,
  executeReviewWithSemantics,
  type Finding,
  LOCAL_PROVIDERS,
  LocalGitProvider,
  type LocalProviderName,
  listChangedFiles,
  type ReviewPipelineInput,
  type SemanticRun,
  summarizeFindings,
  toRepoRelative,
} from "@debuggatha/engine";
import { type Command, Option } from "commander";
import { loadConfig } from "../config.js";
import { pathPolicy } from "../path-policy.js";
import {
  exceedsThreshold,
  formatGithub,
  formatGitlab,
  formatMarkdown,
  formatText,
  REPORT_FORMATS,
  type ReportFormat,
  SEVERITY_ORDER,
  type SeverityName,
} from "../report.js";
import { toSarif } from "../sarif.js";
import { CliLogger } from "../utils/logger.js";
import { addAnalyzerOptions, analyzerOptionsFrom, describeRuns } from "./analyzers.js";

interface ReviewOptions {
  all?: boolean;
  diff?: string;
  files?: string[];
  base?: string;
  policy?: string;
  pack?: string[];
  format: ReportFormat;
  failOn: SeverityName | "none";
  output?: string;
  persist: boolean;
  includeBaselined?: boolean;
  changedSince?: string;
  failOnNew?: boolean;
  analyzer?: string[];
  analyzers: boolean;
  analyzerTimeout?: number;
  semantic?: LocalProviderName;
  semanticModel?: string;
  semanticUrl?: string;
  semanticVerify?: boolean;
}

type ReviewOutput = ReturnType<typeof executeReview> & { semantic?: SemanticRun };

function describeSemantic(run: SemanticRun): string[] {
  const who = `${run.provider}${run.model ? ` ${run.model}` : ""}`;
  const lines = [
    `Semantic pass (${who}): ${run.detected} suspected ${run.detected === 1 ? "issue" : "issues"} raised, ${run.rejected} dropped for not matching the code${run.verified > 0 ? `, ${run.verified} findings commented on` : ""}.`,
  ];
  for (const note of run.notes) lines.push(`Semantic pass: ${note}`);
  return lines;
}

/**
 * Detects scope (`--all` > explicit files > explicit diff (or stdin) >
 * `--base` > local git diff > workspace) then delegates the whole pipeline
 * to `@debuggatha/engine`'s `executeReview`, the same one the MCP server
 * runs. What is CLI-specific lives here: options, scope detection, report
 * formats, and exit codes.
 */
export function registerReviewCommand(program: Command) {
  const review = program
    .command("review")
    .description(
      "Review the given files, a diff, or the working tree against the packs that match the repository's stack.",
    )
    .addOption(
      new Option("--all", "Review the whole repository, even with uncommitted changes").conflicts([
        "files",
        "diff",
        "base",
        "changedSince",
      ]),
    )
    .option("--diff <diff>", 'A unified diff to review; "-" reads it from stdin')
    .option("--files <path...>", "Review whole files instead of a diff")
    .option("--base <ref>", "Review the changes since this git ref (e.g. origin/main...HEAD)")
    .addOption(
      new Option(
        "--changed-since <ref>",
        "Review whole files changed since this git ref (e.g. origin/main), including new ones",
      ).conflicts(["files", "diff", "base"]),
    )
    .option(
      "--fail-on-new",
      "Fail only on findings on lines you changed (since --changed-since, --base, or HEAD); the rest are reported but do not affect the exit code",
    )
    .option("--policy <id>", "Assert the review policy id a previous review reported")
    .option("--pack <id...>", "Extra Review Packs on top of the detected ones")
    .addOption(
      new Option("--format <format>", "Report format")
        .choices(REPORT_FORMATS)
        .default("text" satisfies ReportFormat),
    )
    .addOption(
      new Option("--fail-on <severity>", "Lowest severity that makes the run exit with 2")
        .choices([...SEVERITY_ORDER, "none"])
        .default("informational" satisfies SeverityName),
    )
    .option("--output <file>", "Write the report to a file instead of stdout")
    .option("--no-persist", "Do not record findings in .debuggatha/ledger.json")
    .option(
      "--include-baselined",
      "Also report findings the baseline accepted (see `debuggatha baseline`)",
    );
  addAnalyzerOptions(review)
    .addOption(
      new Option(
        "--semantic <provider>",
        "Also ask a model on this machine to read the changed code (ollama or lmstudio)",
      ).choices(LOCAL_PROVIDERS),
    )
    .option("--semantic-model <name>", "The model to ask (default: the first one the runtime has)")
    .option("--semantic-url <url>", "Where the runtime listens; only localhost is accepted")
    .option(
      "--semantic-verify",
      "Also attach the model's opinion to findings the detectors are unsure of (off: not reliable enough on small models)",
    )
    .addHelpText(
      "after",
      `
Analyzers: tools installed on this machine (Ruff, Biome, gitleaks, ktlint...) run as separate
processes and their findings join the review, each labeled with the tool and its license.
Those that only read source run by default; \`--analyzer <id>\` also runs one that executes
project code or uses the network. \`debuggatha analyzers\` shows what is available.

Semantic pass (--semantic ollama): a model running on this machine reads the changed code for
defects no analyzer sees. Its findings are labeled as suspicions with the model that raised
them; each must quote the line it is about, and none is ever closed, hidden, or scored above
medium by a model. Code is sent only to localhost, and never lines that may hold a secret.

With a baseline (\`debuggatha baseline create\`), only findings that are new since it was
recorded are reported and counted toward the exit code.

Silence a finding you accept, in the code itself:
  risky(); // debuggatha-ignore no-eval -- why
  // debuggatha-ignore-next-line eqeqeq -- why
  // debuggatha-ignore-file no-explicit-any -- why

Exit codes:
  0  no finding at or above --fail-on
  1  the review could not run
  2  at least one finding at or above --fail-on`,
    )
    .action(async (options: ReviewOptions, command) => {
      const cwd = program.opts().cwd as string;
      const asJson = Boolean(program.opts().json);
      const base = command.logger as CliLogger;
      const quiet = new CliLogger({ json: false, verbose: false, quiet: true });
      const fromDefault = (name: string) => command.getOptionValueSource(name) === "default";
      let format: ReportFormat = asJson ? "json" : options.format;
      // Machine formats own stdout, so progress messages are silenced for them.
      let logger = format === "text" ? base : quiet;

      try {
        // What the flags leave unsaid comes from the repository's config, then the defaults.
        const loaded = loadConfig(cwd);
        const configured = loaded.config.review;
        if (!asJson && fromDefault("format") && configured.format) {
          format = configured.format as ReportFormat;
          logger = format === "text" ? base : quiet;
        }
        const failOn = (
          fromDefault("failOn") && configured.failOn ? configured.failOn : options.failOn
        ) as SeverityName | "none";
        for (const warning of loaded.warnings) logger.warn(warning);

        const git = new LocalGitProvider(cwd);
        const filePaths = options.files?.map((file) => resolve(cwd, file));
        let diff = options.diff === "-" ? readStdin() : options.diff;
        let changedFiles: string[] | undefined;

        if (options.all) {
          logger.info("Reviewing the whole repository.");
        } else if (options.changedSince) {
          const changes = listChangedFiles(cwd, options.changedSince);
          changedFiles = [...changes.changed, ...changes.deleted];
          logger.info(
            `Reviewing ${changedFiles.length} files changed since ${options.changedSince}.`,
          );
        } else if (!filePaths && !diff) {
          diff = await git.getDiff(options.base);
          if (options.base && !diff) {
            throw new Error(`No changes found against "${options.base}".`);
          }
          logger.info(
            diff
              ? "Reviewing the changes in your working tree (use --all for the whole repository)."
              : "No changes detected; reviewing the whole repository.",
          );
        }

        if (changedFiles && changedFiles.length === 0) {
          const note = `No changes since ${options.changedSince}.`;
          const empty = renderEmpty(format, cwd, note);
          if (options.output) writeFileSync(options.output, `${empty}\n`);
          else if (empty) console.log(empty);
          logger.success(note);
          return;
        }

        const scope = options.all
          ? ({ kind: "workspace" } as const)
          : changedFiles
            ? ({ kind: "files", paths: changedFiles.map((file) => resolve(cwd, file)) } as const)
            : filePaths
              ? ({ kind: "files", paths: filePaths } as const)
              : diff
                ? ({ kind: "diff", base: options.base, diff } as const)
                : ({ kind: "workspace" } as const);

        const reviewInput: ReviewPipelineInput = {
          rootDir: cwd,
          scope,
          depth: "full",
          packIds: [...(configured.packs ?? []), ...(options.pack ?? [])],
          policyId: options.policy,
          sourceName: "debuggatha-cli",
          persist: options.persist,
          includeBaselined: Boolean(options.includeBaselined),
          analyzers: analyzerOptionsFrom(options, loaded.config.analyzers),
        };
        const reviewed: ReviewOutput = options.semantic
          ? await executeReviewWithSemantics(reviewInput, {
              provider: await createLocalProvider(options.semantic, {
                ...(options.semanticModel ? { model: options.semanticModel } : {}),
                ...(options.semanticUrl ? { baseUrl: options.semanticUrl } : {}),
              }),
              verify: Boolean(options.semanticVerify),
            })
          : executeReview(reviewInput);

        // A monorepo's config can leave paths out and hold others to a different bar.
        const policy = pathPolicy(reviewed.result.findings, loaded, cwd);
        const previous = reviewed.result.summary;
        const output: ReviewOutput =
          policy.ignored === 0
            ? reviewed
            : {
                ...reviewed,
                result: {
                  ...reviewed.result,
                  findings: policy.kept,
                  summary: summarizeFindings(policy.kept, {
                    durationMs: previous.durationMs,
                    appliedPacks: previous.appliedPacks,
                    appliedPolicies: previous.appliedPolicies,
                  }),
                },
              };
        const { result, baselined, suppressedInline } = output;

        // With --fail-on-new only what this change touched can fail the run.
        let counted: readonly Finding[] = result.findings;
        if (options.failOnNew) {
          const ref = options.changedSince ?? options.base ?? "HEAD";
          counted = await onChangedLines(cwd, ref, result.findings);
        }
        const outside = result.findings.length - counted.length;

        const notes = [
          ...describeRuns(output.analyzers),
          ...(output.semantic ? describeSemantic(output.semantic) : []),
        ];
        const uncountedNote =
          outside > 0
            ? `${outside} more ${outside === 1 ? "finding is" : "findings are"} outside the lines this change touched and ${outside === 1 ? "does" : "do"} not affect the result.`
            : undefined;
        const report = render(format, cwd, output, options, {
          notes,
          ...(uncountedNote ? { uncounted: uncountedNote } : {}),
          counted: counted.length,
        });
        if (options.output) {
          writeFileSync(options.output, `${report}\n`);
          logger.info(`Report written to ${options.output}.`);
        } else if (report) {
          console.log(report);
        }

        for (const line of notes) logger.info(line);
        logger.success(`Review complete. ${result.findings.length} findings.`);
        if (uncountedNote) logger.info(uncountedNote);
        if (policy.ignored > 0) {
          logger.info(`${policy.ignored} findings under paths the config ignores are left out.`);
        }
        if (baselined > 0) {
          logger.info(
            `${baselined} findings accepted by the baseline are hidden (--include-baselined shows them).`,
          );
        }
        if (suppressedInline.length > 0) {
          logger.info(`${suppressedInline.length} findings hidden by debuggatha-ignore comments.`);
        }
        const fails = counted.some((finding) => {
          const bar = policy.failOnFor(finding.locations[0]?.file ?? "") ?? failOn;
          return exceedsThreshold([finding], bar as SeverityName | "none");
        });
        if (fails) process.exitCode = 2;
      } catch (err: unknown) {
        if (format === "text") {
          base.error("Review failed.", err);
        } else {
          process.stderr.write(
            `Review failed: ${err instanceof Error ? err.message : String(err)}\n`,
          );
        }
        process.exit(1);
      }
    });
}

/** The untracked files git would add: wholly new to the change, with no diff lines of their own. */
function untrackedFiles(cwd: string): Set<string> {
  try {
    const listed = execFileSync("git", ["ls-files", "-z", "--others", "--exclude-standard"], {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    return new Set(listed.split("\0").filter(Boolean));
  } catch {
    return new Set();
  }
}

/** The findings that sit on lines added since `ref` (or in files that are new); a finding with no line counts when its file changed. */
async function onChangedLines(
  cwd: string,
  ref: string,
  findings: readonly Finding[],
): Promise<Finding[]> {
  const diff = await new LocalGitProvider(cwd).getDiff(ref);
  const added = diff ? addedLinesByFile(diff) : new Map<string, Set<number>>();
  const fresh = untrackedFiles(cwd);
  return findings.filter((finding) => {
    const location = finding.locations[0];
    if (!location) return false;
    const file = toRepoRelative(cwd, location.file);
    if (fresh.has(file)) return true;
    const lines = added.get(file);
    if (!lines) return false;
    const start = location.lines?.start;
    return start === undefined || lines.has(start);
  });
}

function renderEmpty(format: ReportFormat, cwd: string, note: string): string {
  switch (format) {
    case "json":
      return JSON.stringify(
        { status: "success", reviewResult: { findings: [] }, note, analyzers: [], semantic: null },
        null,
        2,
      );
    case "sarif":
      return JSON.stringify(toSarif([], cwd), null, 2);
    case "markdown":
      return formatMarkdown([], cwd, { notes: [note] });
    case "gitlab":
      return "[]";
    default:
      return "";
  }
}

function render(
  format: ReportFormat,
  cwd: string,
  output: ReviewOutput,
  options: ReviewOptions,
  context: { notes: string[]; uncounted?: string; counted: number },
): string {
  const { result } = output;
  switch (format) {
    case "json":
      return JSON.stringify(
        {
          status: "success",
          reviewResult: result,
          syncReport: output.syncReport,
          suppressedFindings: output.suppressedFindings,
          suppressedInline: output.suppressedInline,
          baselined: output.baselined,
          analyzers: output.analyzers,
          semantic: output.semantic ?? null,
          countedFindings: context.counted,
          persisted: options.persist,
        },
        null,
        2,
      );
    case "sarif":
      return JSON.stringify(toSarif(result.findings, cwd), null, 2);
    case "github":
      return formatGithub(result.findings, cwd);
    case "markdown":
      return formatMarkdown(result.findings, cwd, {
        notes: context.notes,
        ...(context.uncounted ? { uncounted: context.uncounted } : {}),
      });
    case "gitlab":
      return formatGitlab(result.findings, cwd);
    case "text":
      return formatText(result.findings, cwd);
  }
}

function readStdin(): string {
  const text = readFileSync(0, "utf8");
  if (!text.trim()) throw new Error("--diff - expected a diff on stdin, but stdin was empty.");
  return text;
}
