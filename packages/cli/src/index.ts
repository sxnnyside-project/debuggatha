import { Command } from "commander";
import { registerDoctorCommand } from "./commands/doctor.js";
import { registerFindingsCommand } from "./commands/findings.js";
import { registerInitCommand } from "./commands/init.js";
import { registerMemoryCommand } from "./commands/memory.js";
import { registerPacksCommand } from "./commands/packs.js";
import { registerPoliciesCommand } from "./commands/policies.js";
import { registerRepositoryCommand } from "./commands/repository.js";
import { registerReviewCommand } from "./commands/review.js";
import { CliLogger } from "./utils/logger.js";

/**
 * Standalone CLI for CI usage and local developer experience.
 *
 * Architecture Principle: This is purely an application/integration layer.
 * It strictly orchestrates `@debuggatha/core` services without duplicating
 * any business logic, domain constraints, or review generation.
 * (See CLAUDE.md "Deferred (not v1)" / "Epic 6").
 */
export function createCli(): Command {
  const program = new Command();

  program
    .name("debuggatha")
    .description("Debuggatha CLI - Advanced Code Review & Intelligence")
    .version("0.1.0");

  program
    .option("--json", "Output strictly in JSON format", false)
    .option("--verbose", "Enable verbose output", false)
    .option("--quiet", "Suppress non-error human output", false)
    .option("-C, --cwd <dir>", "Run as if started in the specified directory", process.cwd());

  // Global interceptor to setup logger based on parsed options
  program.hook("preAction", (thisCommand, actionCommand) => {
    const options = thisCommand.opts();
    const logger = new CliLogger({
      json: !!options.json,
      verbose: !!options.verbose,
      quiet: !!options.quiet,
    });

    // Attach logger to the command object so subcommands can access it
    (actionCommand as unknown as { logger: CliLogger }).logger = logger;
  });

  // Register Commands
  registerInitCommand(program);
  registerReviewCommand(program);
  registerFindingsCommand(program);
  registerMemoryCommand(program);
  registerRepositoryCommand(program);
  registerPacksCommand(program);
  registerPoliciesCommand(program);
  registerDoctorCommand(program);

  return program;
}

// Ensure it only runs automatically if invoked as a script
if (import.meta.url === `file://${process.argv[1]}`) {
  const cli = createCli();
  cli.parse(process.argv);
}
