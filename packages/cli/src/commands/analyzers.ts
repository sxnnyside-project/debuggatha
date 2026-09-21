import {
  ADAPTERS,
  type AnalyzerOptions,
  type AnalyzerRun,
  inventoryAnalyzers,
} from "@debuggatha/engine";
import { type Command, InvalidArgumentError, Option } from "commander";
import pc from "picocolors";
import { type DebuggathaConfig, loadConfig } from "../config.js";
import type { CliLogger } from "../utils/logger.js";

export interface AnalyzerFlags {
  analyzer?: string[];
  /** Commander's `--no-analyzers` negation: `false` when given. */
  analyzers: boolean;
  analyzerTimeout?: number;
}

const IDS = ADAPTERS.map((adapter) => adapter.id);

/** The flags every command that runs a review takes, so a baseline and a review always agree. */
export function addAnalyzerOptions(command: Command): Command {
  return command
    .addOption(
      new Option(
        "--analyzer <id...>",
        "Also run these analyzers, including ones that run project code or use the network",
      ).argParser((value: string, previous: string[] | undefined) => {
        if (!IDS.includes(value)) {
          throw new InvalidArgumentError(`Unknown analyzer "${value}". Known: ${IDS.join(", ")}.`);
        }
        return [...(previous ?? []), value];
      }),
    )
    .option("--no-analyzers", "Run only the built-in detectors")
    .addOption(
      new Option("--analyzer-timeout <seconds>", "Seconds one analyzer may run")
        .argParser((value: string) => {
          const seconds = Number(value);
          if (!Number.isFinite(seconds) || seconds <= 0) {
            throw new InvalidArgumentError("Expected a number of seconds above 0.");
          }
          return seconds;
        })
        .default(undefined),
    );
}

/**
 * What a run allows: the flags, then the repository's config for what a flag did not say.
 * The config can only narrow (turn analyzers off, skip some, set a time limit); enabling
 * stays with the flags.
 */
export function analyzerOptionsFrom(
  flags: AnalyzerFlags,
  config: DebuggathaConfig["analyzers"] = {},
): AnalyzerOptions {
  if (!flags.analyzers) return { mode: "off" };
  const enable = flags.analyzer ?? [];
  if (config.mode === "off" && enable.length === 0) return { mode: "off" };
  const timeoutSeconds = flags.analyzerTimeout ?? config.timeoutSeconds;
  return {
    mode: "auto",
    enable,
    ...(config.disable?.length ? { disable: config.disable } : {}),
    enableHint: (id) => `enable it with --analyzer ${id}`,
    ...(timeoutSeconds ? { timeoutSeconds } : {}),
  };
}

/** Silent when there is nothing to say: a tool the repository does not use is not news. */
const UNREMARKABLE = /nothing it analyzes|no matching files|turned off/;

export function describeRuns(runs: readonly AnalyzerRun[]): string[] {
  return runs
    .filter((run) => run.status !== "skipped" || !UNREMARKABLE.test(run.reason ?? ""))
    .map((run) => {
      if (run.status === "ran") {
        return `${run.name}${run.version ? ` ${run.version}` : ""} ran: ${run.findings} findings.`;
      }
      if (run.status === "failed") {
        return `${run.name} failed (${run.reason}); its findings are not part of this review.`;
      }
      return `${run.name} skipped: ${run.reason}.`;
    });
}

export function registerAnalyzersCommand(program: Command) {
  addAnalyzerOptions(
    program
      .command("analyzers")
      .description(
        "List the external analyzers Debuggatha can run: what is installed here, its license, and what a review would run",
      ),
  ).action((flags: AnalyzerFlags, command) => {
    const logger = command.logger as CliLogger;
    const cwd = program.opts().cwd as string;
    const entries = inventoryAnalyzers(
      cwd,
      analyzerOptionsFrom(flags, loadConfig(cwd).config.analyzers),
    );

    if (logger.isJson) {
      logger.json({ status: "success", analyzers: entries });
      return;
    }
    for (const entry of entries) {
      const state = entry.willRun
        ? pc.green("will run")
        : !entry.applies
          ? pc.dim("not used here")
          : entry.installed
            ? pc.yellow("not run")
            : pc.dim("not installed");
      const version = entry.version ? ` ${entry.version}` : "";
      logger.log(
        `${pc.bold(entry.id)}${version}  ${state}  ${pc.dim(`${entry.license} · ${entry.covers}`)}`,
      );
      if (entry.reason && entry.installed && entry.applies) logger.log(`  ${pc.dim(entry.reason)}`);
      else if (!entry.installed && entry.applies) logger.log(`  ${pc.dim(entry.homepage)}`);
    }
    logger.info(
      "Debuggatha runs these tools as separate processes; it does not include or install them. Each is under its own license.",
    );
  });
}
