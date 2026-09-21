import { existsSync, readFileSync } from "node:fs";
import { dirname, join, parse } from "node:path";
import { z } from "zod";
import { REPORT_FORMATS, SEVERITY_ORDER } from "./report.js";

/**
 * `.debuggatha/config.json`: the settings a team wants every run of the repository to share.
 *
 * It is a file inside the repository, so anyone who can change the repository can change it.
 * That is why it can only make a review stricter or quieter, never more powerful: it cannot
 * enable an analyzer that runs project code or uses the network, and it cannot turn on the
 * semantic pass (which sends code to a model). Those stay with the person running the review,
 * as flags, so a pull request cannot switch them on for whoever reviews it.
 */
const analyzers = z
  .object({
    mode: z.enum(["auto", "off"]).optional(),
    /** Analyzers to skip in this repository, by id. */
    disable: z.array(z.string()).optional(),
    timeoutSeconds: z.number().positive().optional(),
  })
  .strict();

const review = z
  .object({
    /** Lowest severity that fails a run. */
    failOn: z.enum([...SEVERITY_ORDER, "none"]).optional(),
    format: z.enum(REPORT_FORMATS as [string, ...string[]]).optional(),
    /** Extra Review Packs on top of the detected ones. */
    packs: z.array(z.string()).optional(),
  })
  .strict();

const schema = z
  .object({
    // `init` used to write a comment key and two settings nothing read.
    "//": z.string().optional(),
    review: review
      .extend({
        defaultPolicy: z.unknown().optional(),
        defaultDepth: z.unknown().optional(),
      })
      .optional(),
    analyzers: analyzers.optional(),
    /** Paths whose findings are left out of the report and the exit code (generated code, vendored packages). */
    ignore: z.array(z.string()).optional(),
    /** A different bar for some paths, so one package can be held to more (or less) than the rest of a monorepo. */
    overrides: z
      .array(
        z
          .object({
            paths: z.array(z.string()).min(1),
            failOn: z.enum([...SEVERITY_ORDER, "none"]),
          })
          .strict(),
      )
      .optional(),
  })
  .strict();

export interface PathOverride {
  paths: string[];
  failOn: string;
}

export interface DebuggathaConfig {
  ignore: string[];
  overrides: PathOverride[];
  review: {
    failOn?: string | undefined;
    format?: string | undefined;
    packs?: string[] | undefined;
  };
  analyzers: {
    mode?: "auto" | "off" | undefined;
    disable?: string[] | undefined;
    timeoutSeconds?: number | undefined;
  };
}

export interface LoadedConfig {
  config: DebuggathaConfig;
  /** The file it came from, or `undefined` when the repository has none. */
  path: string | undefined;
  /** The directory `ignore` and `overrides` patterns are relative to: where the config's `.debuggatha` folder is. */
  root: string;
  warnings: string[];
}

const REFUSED: Record<string, string> = {
  semantic:
    "the semantic pass sends code to a model, so only the person running the review can turn it on (--semantic)",
  enable:
    "enabling an analyzer that runs project code or uses the network is the decision of the person running the review (--analyzer)",
};

export const CONFIG_FILE = join(".debuggatha", "config.json");

/** The nearest config at or above `cwd`, so a package inside a monorepo finds the repository's. */
export function findConfig(cwd: string): string | undefined {
  for (let dir = cwd; ; dir = dirname(dir)) {
    const candidate = join(dir, CONFIG_FILE);
    if (existsSync(candidate)) return candidate;
    // A repository root ends the search: another repository's config is not ours.
    if (existsSync(join(dir, ".git")) || dir === parse(dir).root) return undefined;
  }
}

function describeIssue(issue: z.core.$ZodIssue): string {
  const where = issue.path.join(".") || "(top level)";
  if (issue.code === "unrecognized_keys") {
    return issue.keys
      .map((key) => {
        const reason = REFUSED[key];
        return reason
          ? `"${[...issue.path, key].join(".")}" is not allowed in the file: ${reason}.`
          : `"${[...issue.path, key].join(".")}" is not a setting. Known: review.failOn, review.format, review.packs, analyzers.mode, analyzers.disable, analyzers.timeoutSeconds, ignore, overrides.`;
      })
      .join(" ");
  }
  return `${where}: ${issue.message}`;
}

export function loadConfig(cwd: string): LoadedConfig {
  const empty: DebuggathaConfig = { review: {}, analyzers: {}, ignore: [], overrides: [] };
  const path = findConfig(cwd);
  if (!path) return { config: empty, path: undefined, root: cwd, warnings: [] };

  let raw: unknown;
  try {
    raw = JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new Error(
      `${path} is not valid JSON (${error instanceof Error ? error.message : String(error)}).`,
    );
  }

  const parsed = schema.safeParse(raw);
  if (!parsed.success) {
    throw new Error(`${path} has a problem: ${parsed.error.issues.map(describeIssue).join(" ")}`);
  }

  const warnings: string[] = [];
  const legacy = parsed.data.review as
    | { defaultPolicy?: unknown; defaultDepth?: unknown }
    | undefined;
  if (legacy && (legacy.defaultPolicy !== undefined || legacy.defaultDepth !== undefined)) {
    warnings.push(
      "review.defaultPolicy and review.defaultDepth were never read and are ignored; remove them.",
    );
  }
  const { defaultPolicy: _p, defaultDepth: _d, ...review } = parsed.data.review ?? {};
  return {
    config: {
      review: review as DebuggathaConfig["review"],
      analyzers: parsed.data.analyzers ?? {},
      ignore: parsed.data.ignore ?? [],
      overrides: parsed.data.overrides ?? [],
    },
    path,
    // Patterns are written from the directory the config lives in.
    root: dirname(dirname(path)),
    warnings,
  };
}
