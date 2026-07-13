import { executeReview } from "@debuggatha/core";
import { LocalGitProvider } from "@debuggatha/infrastructure";
import type { Command } from "commander";
import type { CliLogger } from "../utils/logger.js";

/**
 * Detects scope (explicit files > explicit diff > local git diff >
 * workspace) then delegates the entire nine-step orchestration to
 * `@debuggatha/core`'s `executeReview` — the same pipeline
 * `@debuggatha/mcp` runs (Epic 11). This command used to hand-roll that
 * sequence itself; it now only does what's genuinely CLI-specific:
 * option parsing, git-diff auto-detection, and result formatting.
 */
export function registerReviewCommand(program: Command) {
  program
    .command("review")
    .description("Run a smart review. Detects context automatically or delegates to subcommands.")
    .option("--diff <string>", "Explicitly review a specific diff")
    .option("--files <path...>", "Review specific files instead of a diff or the whole workspace")
    .option("--policy <id>", "Override default policy")
    .option("--pack <id...>", "Add specific review packs")
    .action(async (options, command) => {
      const logger = command.logger as CliLogger;
      const cwd = program.opts().cwd as string;
      logger.info("Running smart review...");

      try {
        const filePaths: string[] | undefined =
          options.files && options.files.length > 0 ? options.files : undefined;

        let diff: string | undefined = options.diff;

        if (!filePaths && !diff) {
          logger.debug("Checking for git diff...");
          const git = new LocalGitProvider(cwd);
          diff = await git.getDiff();
          logger.info(
            diff
              ? "Detected local git diff. Running diff review."
              : "No git diff detected. Running workspace review.",
          );
        } else if (filePaths) {
          logger.info(`Running review scoped to ${filePaths.length} file(s).`);
        } else {
          logger.info("Running explicit diff review.");
        }

        const scope = filePaths
          ? ({ kind: "files", paths: filePaths } as const)
          : diff
            ? ({ kind: "diff", base: undefined, diff } as const)
            : ({ kind: "workspace" } as const);

        const { result, syncReport } = executeReview({
          rootDir: cwd,
          scope,
          depth: "full",
          packIds: options.pack || [],
          policyId: options.policy,
          sourceName: "debuggatha-cli",
        });

        logger.success(`Review complete. ${result.findings.length} findings found.`);
        if (logger.isJson) {
          logger.json({ status: "success", reviewResult: result, syncReport });
        } else {
          logger.info(
            `Sync Report: ${syncReport.newEntryIds.length} created, ${syncReport.autoResolvedEntryIds.length} resolved, ${syncReport.unchangedEntryIds.length + syncReport.changedEntryIds.length} unresolved.`,
          );
        }
        // Distinct exit codes (Epic 16B): 0 = success, no active findings;
        // 2 = success, but the review found something; 1 (below) = the
        // review itself failed to run. Lets CI treat "found findings" and
        // "crashed" differently instead of collapsing both into `1`.
        if (result.findings.length > 0) {
          process.exitCode = 2;
        }
      } catch (err: unknown) {
        logger.error("Review failed.", err);
        process.exit(1);
      }
    });
}
