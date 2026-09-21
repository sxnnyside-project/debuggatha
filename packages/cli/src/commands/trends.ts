import { isActiveFindingStatus, type LedgerEntry, loadLedger } from "@debuggatha/engine";
import { type Command, InvalidArgumentError, Option } from "commander";
import pc from "picocolors";
import { SEVERITY_ORDER } from "../report.js";
import type { CliLogger } from "../utils/logger.js";

const DAY_MS = 24 * 60 * 60 * 1000;

export interface Trends {
  /** The window looked at, in days. */
  days: number;
  open: { total: number; bySeverity: Record<string, number> };
  /** Findings first seen and findings closed in the window, per week (oldest first). */
  weeks: { start: string; introduced: number; resolved: number }[];
  introduced: number;
  resolved: number;
  /** Average days from first seen to resolved, over findings closed in the window; `undefined` when none were. */
  meanDaysToResolve: number | undefined;
  /** Where the open findings are. */
  topFiles: { file: string; open: number }[];
  topRules: { ruleId: string; open: number }[];
}

const isoDay = (ms: number) => new Date(ms).toISOString().slice(0, 10);

function tally<T extends string>(items: readonly T[], limit: number) {
  const counts = new Map<T, number>();
  for (const item of items) counts.set(item, (counts.get(item) ?? 0) + 1);
  return [...counts].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).slice(0, limit);
}

/** What the ledger's history says about how a repository's findings are moving. */
export function computeTrends(
  entries: readonly LedgerEntry[],
  days: number,
  now = Date.now(),
): Trends {
  const since = now - days * DAY_MS;
  const open = entries.filter((entry) => isActiveFindingStatus(entry.status));
  const bySeverity: Record<string, number> = {};
  for (const entry of open) {
    const severity = entry.latestFinding.severity;
    bySeverity[severity] = (bySeverity[severity] ?? 0) + 1;
  }

  const weekCount = Math.max(1, Math.ceil(days / 7));
  const weeks = Array.from({ length: weekCount }, (_, index) => ({
    start: isoDay(since + index * 7 * DAY_MS),
    introduced: 0,
    resolved: 0,
  }));
  const weekOf = (ms: number) => Math.min(weekCount - 1, Math.floor((ms - since) / (7 * DAY_MS)));

  let introduced = 0;
  let resolved = 0;
  const durations: number[] = [];
  for (const entry of entries) {
    const first = entry.history[0] ? Date.parse(entry.history[0].timestamp) : undefined;
    if (first !== undefined && first >= since) {
      introduced += 1;
      const week = weeks[weekOf(first)];
      if (week) week.introduced += 1;
    }
    // The last time it was closed, if it is closed now.
    const closed =
      entry.status === "resolved"
        ? [...entry.history].reverse().find((event) => event.newState === "resolved")
        : undefined;
    const closedAt = closed ? Date.parse(closed.timestamp) : undefined;
    if (closedAt !== undefined && closedAt >= since) {
      resolved += 1;
      const week = weeks[weekOf(closedAt)];
      if (week) week.resolved += 1;
      if (first !== undefined) durations.push((closedAt - first) / DAY_MS);
    }
  }

  return {
    days,
    open: { total: open.length, bySeverity },
    weeks,
    introduced,
    resolved,
    meanDaysToResolve:
      durations.length === 0
        ? undefined
        : durations.reduce((sum, value) => sum + value, 0) / durations.length,
    topFiles: tally(
      open.map((entry) => entry.fingerprint.file),
      5,
    ).map(([file, count]) => ({ file, open: count })),
    topRules: tally(
      open.map((entry) => entry.fingerprint.ruleId ?? entry.fingerprint.category),
      5,
    ).map(([ruleId, count]) => ({ ruleId, open: count })),
  };
}

function parseDays(value: string): number {
  const match = /^(\d+)(d|w)?$/.exec(value);
  if (!match) throw new InvalidArgumentError('Expected a window like "30d" or "8w".');
  const amount = Number(match[1]);
  const days = match[2] === "w" ? amount * 7 : amount;
  if (days < 1) throw new InvalidArgumentError("The window has to be at least a day.");
  return days;
}

export function registerTrendsCommand(program: Command) {
  program
    .command("trends")
    .description(
      "How the repository's findings are moving: open now, introduced and resolved per week, and how long fixes take",
    )
    .addOption(
      new Option("--since <window>", "How far back to look, e.g. 30d or 8w")
        .argParser(parseDays)
        .default(30, "30d"),
    )
    .action((options: { since: number }, command) => {
      const logger = command.logger as CliLogger;
      const cwd = program.opts().cwd as string;
      try {
        const ledger = loadLedger(cwd);
        const trends = computeTrends(ledger.entries, options.since);
        if (logger.isJson) {
          logger.json({ status: "success", trends });
          return;
        }
        if (ledger.entries.length === 0) {
          logger.info(
            "The ledger is empty. Run `debuggatha review` (without --no-persist) to start recording findings.",
          );
          return;
        }
        const severities = SEVERITY_ORDER.slice()
          .reverse()
          .filter((severity) => trends.open.bySeverity[severity])
          .map((severity) => `${trends.open.bySeverity[severity]} ${severity}`);
        logger.log(
          `${pc.bold(String(trends.open.total))} open now${severities.length > 0 ? `: ${severities.join(", ")}` : ""}`,
        );
        logger.log(
          `Last ${trends.days} days: ${trends.introduced} introduced, ${trends.resolved} resolved${
            trends.meanDaysToResolve === undefined
              ? ""
              : `, fixed in ${trends.meanDaysToResolve.toFixed(1)} days on average`
          }`,
        );
        logger.log("");
        logger.log(pc.dim("week of     introduced  resolved"));
        for (const week of trends.weeks) {
          logger.log(
            `${week.start}  ${String(week.introduced).padStart(10)}  ${String(week.resolved).padStart(8)}`,
          );
        }
        if (trends.topFiles.length > 0) {
          logger.log("");
          logger.log(pc.dim("most open findings by file"));
          for (const { file, open } of trends.topFiles) {
            logger.log(`${String(open).padStart(5)}  ${file}`);
          }
          logger.log(pc.dim("by rule"));
          for (const { ruleId, open } of trends.topRules) {
            logger.log(`${String(open).padStart(5)}  ${ruleId}`);
          }
        }
      } catch (err: unknown) {
        logger.error("Could not read the ledger.", err);
        process.exit(1);
      }
    });
}
