import { defaultCapabilityRegistry } from "@debuggatha/engine";
import type { Command } from "commander";
import type { CliLogger } from "../utils/logger.js";

export function registerPacksCommand(program: Command) {
  program
    .command("packs")
    .description("List available Review Packs")
    .action(async (_options, command) => {
      const logger = command.logger as CliLogger;
      logger.info("Listing available Review Packs...");
      const packs = defaultCapabilityRegistry.listPacks();

      if (logger.isJson) {
        logger.json(packs);
      } else {
        logger.success(`Found ${packs.length} Review Packs.`);
        for (const pack of packs) {
          logger.log(`- [${pack.id}] (v${pack.version}) ${pack.displayName}`);
        }
      }
    });
}
