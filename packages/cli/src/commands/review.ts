import {
  assemblePolicy,
  buildRepositoryContext,
  createReviewRequest,
  createReviewResult,
  createReviewSession,
  type Finding,
  loadLedger,
  type ReviewScope,
  reviewArchitecture,
  reviewDiff,
  reviewFiles,
  saveLedger,
  synchronizeReviewResult,
  transitionSession,
} from "@debuggatha/core";
import { LocalGitProvider } from "@debuggatha/infrastructure";
import type { Command } from "commander";
import type { CliLogger } from "../utils/logger.js";

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
        const repoContext = await buildRepositoryContext(cwd);
        logger.debug(`Repository context built for ${repoContext.rootDir}`);

        const filePaths: string[] | undefined =
          options.files && options.files.length > 0 ? options.files : undefined;

        let diff: string | undefined = options.diff;
        let isDiff = !!diff;

        if (!filePaths) {
          if (!diff) {
            logger.debug("Checking for git diff...");
            const git = new LocalGitProvider(cwd);
            diff = await git.getDiff();
            if (diff) {
              logger.info("Detected local git diff. Running diff review.");
              isDiff = true;
            } else {
              logger.info("No git diff detected. Running workspace review.");
              isDiff = false;
            }
          } else {
            logger.info("Running explicit diff review.");
          }
        } else {
          logger.info(`Running review scoped to ${filePaths.length} file(s).`);
        }

        const requestScope: ReviewScope = filePaths
          ? { kind: "files", paths: filePaths }
          : isDiff
            ? { kind: "diff", base: undefined }
            : { kind: "workspace" };

        const requestInput = {
          scope: requestScope,
          depth: "full" as const,
          requestedPackIds: options.pack || [],
          ...(options.policy !== undefined ? { requestedPolicyId: options.policy } : {}),
        };

        const request = createReviewRequest(requestInput);
        const policy = assemblePolicy(repoContext, request);

        let session = createReviewSession({
          request,
          repositoryContext: repoContext,
          selectedPolicy: { id: policy.id },
          selectedPacks: policy.packRefs,
        });

        session = transitionSession(session, "prepared");
        session = transitionSession(session, "running", {
          source: { kind: "deterministic-analyzer", name: "debuggatha-cli" },
          startedAt: new Date().toISOString(),
        });

        let findings: Finding[];
        try {
          if (filePaths) {
            findings = reviewFiles(filePaths, policy);
          } else if (isDiff) {
            findings = reviewDiff(diff as string, policy);
          } else {
            findings = reviewArchitecture(repoContext.rootDir, policy);
          }
        } catch (err: unknown) {
          session = transitionSession(session, "failed", {
            completedAt: new Date().toISOString(),
            error: err instanceof Error ? err.message : String(err),
          });
          throw err;
        }

        session = transitionSession(session, "completed", {
          completedAt: new Date().toISOString(),
        });
        const result = createReviewResult({ session, findings });

        const ledger = loadLedger(repoContext.rootDir);
        const syncScope = filePaths
          ? { kind: "files" as const, files: filePaths }
          : isDiff
            ? {
                kind: "files" as const,
                files: [...new Set(findings.flatMap((f) => f.locations.map((l) => l.file)))],
              }
            : { kind: "workspace" as const };

        const { ledger: updatedLedger, report } = synchronizeReviewResult(
          ledger,
          result,
          repoContext,
          syncScope,
        );
        saveLedger(updatedLedger);

        logger.success(`Review complete. ${findings.length} findings found.`);
        if (logger.isJson) {
          logger.json({ status: "success", reviewResult: result, syncReport: report });
        } else {
          logger.info(
            `Sync Report: ${report.newEntryIds.length} created, ${report.autoResolvedEntryIds.length} resolved, ${report.unchangedEntryIds.length + report.changedEntryIds.length} unresolved.`,
          );
        }
      } catch (err: unknown) {
        logger.error("Review failed.", err);
        process.exit(1);
      }
    });
}
