import fs from "node:fs";
import path from "node:path";
import { buildRepositoryContext } from "@debuggatha/core";
import type { Command } from "commander";
import type { CliLogger } from "../utils/logger.js";

export function registerDoctorCommand(program: Command) {
  program
    .command("doctor")
    .description("Verify repository health, configuration, and environment")
    .action(async (_options, command) => {
      const logger = command.logger as CliLogger;
      const cwd = program.opts().cwd as string;
      logger.info("Running diagnostics...");

      const diagnostics = {
        environment: { nodeVersion: process.version },
        repository: { isGit: false, hasDebuggathaDir: false },
        context: { valid: false, error: null as string | null },
      };

      // 1. Environment
      logger.success(`Node Version: ${process.version}`);

      // 2. Repository checks
      const isGit = fs.existsSync(path.join(cwd, ".git"));
      diagnostics.repository.isGit = isGit;
      if (isGit) logger.success("Git repository detected.");
      else logger.warn("No .git directory found. Some features may be limited.");

      const hasDebuggathaDir = fs.existsSync(path.join(cwd, ".debuggatha"));
      diagnostics.repository.hasDebuggathaDir = hasDebuggathaDir;
      if (hasDebuggathaDir) logger.success(".debuggatha directory found.");
      else logger.warn("No .debuggatha directory found. Run 'debuggatha init'.");

      // 3. Context validation
      try {
        await buildRepositoryContext(cwd);
        diagnostics.context.valid = true;
        logger.success("Repository Context successfully built.");
      } catch (err: unknown) {
        diagnostics.context.error = err instanceof Error ? err.message : String(err);
        logger.error("Failed to build Repository Context.", err);
      }

      const isHealthy = diagnostics.context.valid;

      if (logger.isJson) {
        logger.json({ status: isHealthy ? "success" : "error", diagnostics });
      } else {
        if (isHealthy) logger.success("Diagnostics passed. Debuggatha is ready to roll!");
        else logger.error("Diagnostics failed. See above for details.");
      }

      if (!isHealthy) {
        process.exit(1);
      }
    });
}
