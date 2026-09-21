import {
  type FindingLifecycleStatus,
  getEntry,
  listEntries,
  loadLedger,
  saveLedger,
  summarizeLedger,
  updateFindingStatus,
} from "@debuggatha/engine";
import type { Command } from "commander";
import type { CliLogger } from "../utils/logger.js";

export function registerFindingsCommand(program: Command) {
  const findings = program.command("findings").description("Manage and query the Findings Ledger");

  findings
    .command("list")
    .description("List open findings")
    .action(async (_options, command) => {
      const logger = command.logger as CliLogger;
      const cwd = program.opts().cwd as string;
      try {
        const ledger = loadLedger(cwd);
        const entries = listEntries(ledger, { status: ["open"] });
        if (logger.isJson) {
          logger.json(entries);
        } else {
          logger.info(`Found ${entries.length} open findings.`);
          for (const entry of entries) {
            logger.log(
              `[${entry.id}] ${entry.latestFinding.severity.toUpperCase()} - ${entry.latestFinding.category}: ${entry.latestFinding.title}`,
            );
          }
        }
      } catch (err) {
        logger.error("Failed to list findings.", err);
        process.exit(1);
      }
    });

  findings
    .command("show <id>")
    .description("Show details of a specific finding")
    .action(async (id, _options, command) => {
      const logger = command.logger as CliLogger;
      const cwd = program.opts().cwd as string;
      try {
        const ledger = loadLedger(cwd);
        const entry = getEntry(ledger, id);
        if (!entry) {
          logger.error(`Finding ${id} not found.`);
          process.exit(1);
        }
        if (logger.isJson) {
          logger.json(entry);
        } else {
          logger.info(`Finding ID: ${entry.id}`);
          logger.log(`Status: ${entry.status}`);
          logger.log(`Severity: ${entry.latestFinding.severity}`);
          logger.log(`Category: ${entry.latestFinding.category}`);
          logger.log(`Summary: ${entry.latestFinding.title}`);
          const loc = entry.latestFinding.locations && entry.latestFinding.locations[0];
          if (loc && loc.file) {
            logger.log(`Location: ${loc.file}`);
          }
          if (entry.history.length > 0) {
            logger.log("History:");
            for (const event of entry.history) {
              logger.log(
                `  - [${event.timestamp}] ${event.origin.kind}: ${event.newState} (${event.comment || "No comment"})`,
              );
            }
          }
        }
      } catch (err) {
        logger.error(`Failed to show finding ${id}.`, err);
        process.exit(1);
      }
    });

  const transitionCommand = async (id: string, status: FindingLifecycleStatus, command: any) => {
    const logger = command.logger as CliLogger;
    const cwd = program.opts().cwd as string;
    try {
      const ledger = loadLedger(cwd);
      const entry = getEntry(ledger, id);
      if (!entry) {
        logger.error(`Finding ${id} not found.`);
        process.exit(1);
      }
      const updatedLedger = updateFindingStatus(
        ledger,
        id,
        status,
        { kind: "manual", actor: "cli" },
        `CLI transition to ${status}`,
      );
      saveLedger(updatedLedger);
      const updated = getEntry(updatedLedger, id);
      if (logger.isJson) {
        logger.json({ status: "success", entry: updated });
      } else {
        logger.success(`Finding ${id} transitioned to ${status}.`);
      }
    } catch (err) {
      logger.error(`Failed to transition finding ${id}.`, err);
      process.exit(1);
    }
  };

  findings
    .command("resolve <id>")
    .description("Resolve a finding")
    .action(async (id, _options, command) => transitionCommand(id, "resolved", command));

  findings
    .command("dismiss <id>")
    .description("Dismiss a finding")
    .action(async (id, _options, command) => transitionCommand(id, "dismissed", command));

  findings
    .command("reopen <id>")
    .description("Reopen a finding")
    .action(async (id, _options, command) => transitionCommand(id, "open", command));

  findings
    .command("summary")
    .description("Print findings ledger summary")
    .action(async (_options, command) => {
      const logger = command.logger as CliLogger;
      const cwd = program.opts().cwd as string;
      try {
        const ledger = loadLedger(cwd);
        const summary = summarizeLedger(ledger);
        if (logger.isJson) {
          logger.json(summary);
        } else {
          logger.info("Ledger Summary:");
          logger.log(`By Status:`);
          for (const [status, count] of Object.entries(summary.countsByStatus)) {
            logger.log(`  ${status}: ${count}`);
          }
          logger.log(`By Severity (Active):`);
          for (const [severity, count] of Object.entries(summary.activeBySeverity)) {
            logger.log(`  ${severity}: ${count}`);
          }
        }
      } catch (err) {
        logger.error("Failed to generate ledger summary.", err);
        process.exit(1);
      }
    });
}
