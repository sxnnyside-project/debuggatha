import type { Command } from "commander";
import type { CliLogger } from "../utils/logger.js";

export function registerPoliciesCommand(program: Command) {
  program
    .command("policies")
    .description("List available Review Policies")
    .action(async (_options, command) => {
      const logger = command.logger as CliLogger;
      logger.info("Listing available Review Policies...");

      const policies = [
        {
          id: "default",
          description:
            "Dynamically resolves all relevant rules from detected stack criteria and requested packs.",
        },
      ];

      if (logger.isJson) {
        logger.json(policies);
      } else {
        logger.success(`Found ${policies.length} standard policies.`);
        for (const policy of policies) {
          logger.log(`- [${policy.id}]: ${policy.description}`);
        }
        logger.info(
          "Note: Policies in Debuggatha are dynamically assembled based on the repository context at runtime.",
        );
      }
    });
}
