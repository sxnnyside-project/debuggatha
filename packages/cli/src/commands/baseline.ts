import { BASELINE_FILE, clearBaseline, createBaseline, loadBaseline } from "@debuggatha/engine";
import type { Command } from "commander";
import { loadConfig } from "../config.js";
import type { CliLogger } from "../utils/logger.js";
import {
  type AnalyzerFlags,
  addAnalyzerOptions,
  analyzerOptionsFrom,
  describeRuns,
} from "./analyzers.js";

/**
 * Adoption mode: record what a repository has today so reviews report only what
 * is new. Commit `.debuggatha/baseline.json` so the whole team quiets the same
 * findings.
 */
export function registerBaselineCommand(program: Command) {
  const baseline = program
    .command("baseline")
    .description("Accept the findings a repository has today, so reviews report only what is new");

  addAnalyzerOptions(
    baseline
      .command("create")
      .description("Review the whole repository and record every finding as accepted"),
  ).action((flags: AnalyzerFlags, command) => {
    const logger = command.logger as CliLogger;
    const cwd = program.opts().cwd as string;
    try {
      const loaded = loadConfig(cwd);
      for (const warning of loaded.warnings) logger.warn(warning);
      logger.info("Reviewing the whole repository to record a baseline...");
      const {
        baseline: created,
        count,
        analyzers,
      } = createBaseline({
        rootDir: cwd,
        sourceName: "debuggatha-cli",
        // The same packs and analyzers a review will use, or its findings would all look new.
        packIds: loaded.config.review.packs ?? [],
        analyzers: analyzerOptionsFrom(flags, loaded.config.analyzers),
      });
      if (logger.isJson) {
        logger.json({ status: "success", file: BASELINE_FILE, findings: count, analyzers });
        return;
      }
      for (const line of describeRuns(analyzers)) logger.info(line);
      logger.success(
        `Baseline recorded: ${count} findings across ${new Set(created.entries.map((e) => e.file)).size} files.`,
      );
      logger.info(
        `Commit ${BASELINE_FILE}. From now on \`debuggatha review\` reports only new findings; \`--include-baselined\` shows them all.`,
      );
    } catch (err: unknown) {
      logger.error("Could not create the baseline.", err);
      process.exit(1);
    }
  });

  baseline
    .command("show")
    .description("Summarize the recorded baseline")
    .action((_options, command) => {
      const logger = command.logger as CliLogger;
      const cwd = program.opts().cwd as string;
      try {
        const current = loadBaseline(cwd);
        if (!current) {
          logger.info("No baseline. Create one with `debuggatha baseline create`.");
          if (logger.isJson) logger.json({ status: "success", baseline: null });
          return;
        }
        const total = current.entries.reduce((sum, entry) => sum + entry.count, 0);
        if (logger.isJson) {
          logger.json({ status: "success", baseline: current, findings: total });
          return;
        }
        const byRule = new Map<string, number>();
        for (const entry of current.entries) {
          byRule.set(entry.ruleId, (byRule.get(entry.ruleId) ?? 0) + entry.count);
        }
        logger.success(`Baseline from ${current.createdAt}: ${total} accepted findings.`);
        for (const [rule, count] of [...byRule].sort((a, b) => b[1] - a[1])) {
          logger.log(`  ${String(count).padStart(5)}  ${rule}`);
        }
      } catch (err: unknown) {
        logger.error("Could not read the baseline.", err);
        process.exit(1);
      }
    });

  baseline
    .command("clear")
    .description("Delete the baseline; reviews report everything again")
    .action((_options, command) => {
      const logger = command.logger as CliLogger;
      const cwd = program.opts().cwd as string;
      const removed = clearBaseline(cwd);
      if (logger.isJson) return logger.json({ status: "success", removed });
      if (removed) logger.success("Baseline removed.");
      else logger.info("There was no baseline.");
    });
}
