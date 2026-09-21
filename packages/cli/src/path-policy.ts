import { relative } from "node:path";
import type { Finding } from "@debuggatha/engine";
import type { LoadedConfig } from "./config.js";

/**
 * A glob as a monorepo config writes it: `**` crosses directories, `*` and `?` stay inside one,
 * and a pattern with no slash matches that name at any depth (`*.generated.ts`).
 */
export function globToRegExp(glob: string): RegExp {
  const anywhere = !glob.includes("/");
  let source = "";
  for (let i = 0; i < glob.length; i++) {
    const char = glob[i] as string;
    if (char === "*") {
      if (glob[i + 1] === "*") {
        i += 1;
        // `**/` matches zero or more directories.
        if (glob[i + 1] === "/") {
          i += 1;
          source += "(?:.*/)?";
        } else {
          source += ".*";
        }
      } else {
        source += "[^/]*";
      }
    } else if (char === "?") {
      source += "[^/]";
    } else {
      source += char.replace(/[.+^${}()|[\]\\]/g, "\\$&");
    }
  }
  // A directory pattern (`packages/legacy`) covers what is inside it.
  return new RegExp(`^${anywhere ? "(?:.*/)?" : ""}${source}(?:/.*)?$`);
}

export function matchesAny(globs: readonly string[], file: string): boolean {
  return globs.some((glob) => globToRegExp(glob.replace(/^\.\//, "")).test(file));
}

export interface PathPolicy {
  /** Findings that count, with the ones under an ignored path left out. */
  kept: Finding[];
  ignored: number;
  /** The lowest severity that fails a run for a file, or `undefined` for the run's own. */
  failOnFor(file: string): string | undefined;
}

/**
 * What a monorepo's config says about where a finding is. Findings carry paths relative to the
 * directory the review ran in, and patterns are written from the config's directory, so the two
 * are brought to the same base first: a review run inside `packages/api` still matches
 * `packages/api/**`.
 */
export function pathPolicy(
  findings: readonly Finding[],
  loaded: LoadedConfig,
  cwd: string,
): PathPolicy {
  const prefix = relative(loaded.root, cwd).split("\\").join("/");
  const fromConfigRoot = (file: string) => (prefix ? `${prefix}/${file}` : file);
  const { ignore, overrides } = loaded.config;

  const kept = findings.filter((finding) => {
    const file = finding.locations[0]?.file;
    return file === undefined || !matchesAny(ignore, fromConfigRoot(file));
  });
  return {
    kept,
    ignored: findings.length - kept.length,
    failOnFor(file) {
      const path = fromConfigRoot(file);
      return overrides.find((override) => matchesAny(override.paths, path))?.failOn;
    },
  };
}
