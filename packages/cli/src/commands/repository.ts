import { buildRepositoryContext } from "@debuggatha/engine";
import type { Command } from "commander";
import type { CliLogger } from "../utils/logger.js";

export function registerRepositoryCommand(program: Command) {
  program
    .command("repository")
    .description("Inspect Repository Intelligence (stack, criteria, docs)")
    .action(async (_options, command) => {
      const logger = command.logger as CliLogger;
      const cwd = program.opts().cwd as string;
      logger.info(`Analyzing repository at ${cwd}...`);

      try {
        const context = await buildRepositoryContext(cwd);
        if (logger.isJson) {
          logger.json(context);
        } else {
          logger.success("Repository context built successfully.");
          const totalSignals =
            context.stack.languages.length +
            context.stack.frameworks.length +
            context.stack.buildSystems.length +
            context.stack.packageManagers.length +
            context.stack.runtimes.length;
          logger.info(`Stack Evidence: ${totalSignals} signals detected.`);
          logger.info(
            `Dependencies: ${Object.keys(context.dependencies.declaredVersions).length} dependencies found.`,
          );
          logger.info(`Criteria Rules: ${context.criteria.rules.length} custom rules active.`);
          logger.info(`Documentation: ${context.documentation.sources.length} sources indexed.`);
          logger.info(
            `Understanding: ${context.understanding.openQuestions.length} open questions discovered.`,
          );
        }
      } catch (err) {
        logger.error("Failed to build repository context.", err);
        process.exit(1);
      }
    });
}
