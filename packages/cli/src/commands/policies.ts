import {
  assemblePolicy,
  buildRepositoryContext,
  createReviewRequest,
  resolveRequestedPackIds,
} from "@debuggatha/engine";
import type { Command } from "commander";
import type { CliLogger } from "../utils/logger.js";

/**
 * Assembles and displays the real `ReviewPolicy` for the current
 * repository — a Review Policy in this architecture is always dynamic
 * (Rule Resolution against the repository's own detected capabilities/
 * criteria, never a static catalog entry — see `core/knowledge-system`),
 * so this queries the actual registry/resolver instead of printing a
 * fixed list.
 */
export function registerPoliciesCommand(program: Command) {
  program
    .command("policies")
    .description("Show the Review Policy Debuggatha would assemble for this repository right now")
    .action(async (_options, command) => {
      const logger = command.logger as CliLogger;
      const cwd = program.opts().cwd as string;
      logger.info("Assembling the current Review Policy...");

      try {
        const context = buildRepositoryContext(cwd);
        const requestedPackIds = resolveRequestedPackIds([]);
        const request = createReviewRequest({
          scope: { kind: "workspace" },
          depth: "full",
          requestedPackIds,
        });
        const policy = assemblePolicy(context, request);

        if (logger.isJson) {
          logger.json(policy);
        } else {
          logger.success(
            `Policy "${policy.id}" — ${policy.rules.length} rule(s) from ${policy.packRefs.length} pack(s).`,
          );
          for (const packRef of policy.packRefs) {
            logger.log(`- [${packRef.id}] v${packRef.version}`);
          }
          if (policy.conflicts.length > 0) {
            logger.warn(
              `${policy.conflicts.length} rule conflict(s) resolved — see 'debuggatha repository' for detail.`,
            );
          }
          logger.info(
            "Note: this policy is assembled dynamically from the repository's detected capabilities and every registered Review Pack — it is not a fixed catalog entry.",
          );
        }
      } catch (err: unknown) {
        logger.error("Failed to assemble Review Policy.", err);
        process.exit(1);
      }
    });
}
