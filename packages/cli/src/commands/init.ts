import fs from "node:fs";
import path from "node:path";
import type { Command } from "commander";
import type { CliLogger } from "../utils/logger.js";

export function registerInitCommand(program: Command) {
  program
    .command("init")
    .description("Initialize .debuggatha directory and default configuration")
    .action(async (_options, command) => {
      const logger = command.logger as CliLogger;
      const cwd = program.opts().cwd as string;
      const targetDir = path.resolve(cwd, ".debuggatha");

      logger.info(`Initializing Debuggatha in ${cwd}...`);

      try {
        if (!fs.existsSync(targetDir)) {
          fs.mkdirSync(targetDir, { recursive: true });
          logger.success("Created .debuggatha directory.");
        } else {
          logger.info(".debuggatha directory already exists.");
        }

        const configPath = path.join(targetDir, "config.json");
        if (!fs.existsSync(configPath)) {
          fs.writeFileSync(
            configPath,
            JSON.stringify(
              {
                "//": "Debuggatha Configuration",
                review: {
                  defaultPolicy: "default",
                  defaultDepth: "full",
                },
              },
              null,
              2,
            ),
          );
          logger.success("Created default config.json.");
        } else {
          logger.info("config.json already exists.");
        }

        logger.success("Debuggatha initialized successfully.");
        if (logger.isJson) {
          logger.json({ status: "success", initialized: true, targetDir });
        }
      } catch (err) {
        logger.error("Failed to initialize Debuggatha.", err);
        process.exit(1);
      }
    });
}
