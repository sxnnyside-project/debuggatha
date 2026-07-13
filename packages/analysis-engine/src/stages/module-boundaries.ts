import { existsSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { IGNORED_DIRS, readJson, walkSourceFiles } from "../internal/fs-walk.js";
import type { ModuleBoundary, ModuleKind } from "../types.js";

const WORKSPACE_GROUPS: { dir: string; kind: ModuleKind }[] = [
  { dir: "packages", kind: "package" },
  { dir: "apps", kind: "workspace" },
];

/** Common source-tree conventions for a single-package repo without its own sub-packages. */
const FEATURE_LIKE_DIRS = new Set(["features", "modules", "services", "components"]);
const ADAPTER_LIKE_DIRS = new Set(["adapters", "infrastructure", "infra", "gateways", "clients"]);
const LAYER_LIKE_DIRS = new Set(["layers", "core", "application", "presentation", "ui"]);

function kindForDirName(name: string): ModuleKind {
  if (ADAPTER_LIKE_DIRS.has(name)) return "adapter";
  if (LAYER_LIKE_DIRS.has(name)) return "layer";
  if (FEATURE_LIKE_DIRS.has(name)) return "feature";
  return "domain";
}

/**
 * Module Boundaries (Epic 12). Respects whatever Repository Intelligence
 * already established (a monorepo's own `packages/*`/`apps/*` layout is
 * always preferred — "prefer existing project metadata over heuristic
 * parsing") and only falls back to directory-name heuristics
 * (`src/<feature|domain|adapter|layer>/*`) for a single-package repo,
 * where no workspace layout exists to read. Deliberately does not assume
 * one architectural style — a directory that matches no known vocabulary
 * is still a boundary (`kind: "domain"`, the least presumptive default),
 * never dropped.
 */
export function inferModuleBoundaries(rootDir: string): ModuleBoundary[] {
  const boundaries: ModuleBoundary[] = [];

  for (const group of WORKSPACE_GROUPS) {
    const groupDir = join(rootDir, group.dir);
    if (!existsSync(groupDir)) continue;
    let entries: string[];
    try {
      entries = readdirSync(groupDir);
    } catch {
      continue;
    }
    for (const entry of entries) {
      const dir = join(groupDir, entry);
      if (!statSync(dir).isDirectory()) continue;
      const pkg = readJson(join(dir, "package.json"));
      const id = relative(rootDir, dir);
      boundaries.push({
        id,
        dir,
        kind: group.kind,
        name: typeof pkg?.name === "string" ? pkg.name : undefined,
        files: walkSourceFiles(rootDir, dir),
      });
    }
  }

  if (boundaries.length > 0) return boundaries.sort((a, b) => a.id.localeCompare(b.id));

  // No workspace layout — fall back to `src/<name>/*` directory-name heuristics.
  const srcDir = join(rootDir, "src");
  if (!existsSync(srcDir)) return [];
  let entries: string[];
  try {
    entries = readdirSync(srcDir);
  } catch {
    return [];
  }
  for (const entry of entries) {
    if (IGNORED_DIRS.has(entry) || entry.startsWith(".")) continue;
    const dir = join(srcDir, entry);
    if (!statSync(dir).isDirectory()) continue;
    const id = relative(rootDir, dir);
    boundaries.push({
      id,
      dir,
      kind: kindForDirName(entry),
      name: undefined,
      files: walkSourceFiles(rootDir, dir),
    });
  }

  return boundaries.sort((a, b) => a.id.localeCompare(b.id));
}
