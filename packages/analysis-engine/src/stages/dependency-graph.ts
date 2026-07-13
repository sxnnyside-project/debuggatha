import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  dirname as posixDirname,
  join as posixJoin,
  normalize as posixNormalize,
} from "node:path/posix";
import { readJson } from "../internal/fs-walk.js";
import type { DependencyEdge, DependencyGraph, ModuleBoundary } from "../types.js";

const IMPORT_SPECIFIER = /(?:from\s+|require\s*\(\s*|^import\s+)["']([^"']+)["']/gm;

/**
 * Dependency Graph (Epic 12). Nodes are `ModuleBoundary.id`s (Module
 * Boundaries runs first — this stage consumes its output rather than
 * rediscovering boundaries itself, per the pipeline's "every stage
 * should consume previous analysis"). Two edge kinds:
 *
 * - `declared`: read straight from each boundary's own `package.json`
 *   `dependencies`/`devDependencies`/`peerDependencies` — "prefer
 *   existing project metadata over heuristic parsing".
 * - `import`: a source file inside one boundary importing another
 *   boundary's declared package name, or a relative path that resolves
 *   into another boundary's directory — the heuristic fallback for
 *   intra-repo edges no manifest records (e.g. a single-package repo's
 *   `src/domain` importing `src/adapters`).
 */
export function buildDependencyGraph(
  rootDir: string,
  boundaries: ModuleBoundary[],
): DependencyGraph {
  const byName = new Map<string, ModuleBoundary>();
  for (const b of boundaries) if (b.name) byName.set(b.name, b);
  const namesByLength = [...byName.keys()].sort((a, b) => b.length - a.length);

  const byDirPrefix = boundaries
    .map((b) => ({ id: b.id, prefix: `${b.id}/` }))
    .sort((a, b) => b.prefix.length - a.prefix.length);

  function resolveModuleForFile(file: string): string | undefined {
    return byDirPrefix.find((b) => file === b.id || file.startsWith(b.prefix))?.id;
  }

  const edgeKey = (e: Pick<DependencyEdge, "from" | "to" | "kind">) =>
    `${e.kind}:${e.from}->${e.to}`;
  const edgesByKey = new Map<string, DependencyEdge>();

  const addEvidence = (edge: DependencyEdge, file: string, detail: string) => {
    edge.evidence.push({ file, detail });
  };

  const upsertEdge = (
    from: string,
    to: string,
    kind: DependencyEdge["kind"],
    file: string,
    detail: string,
  ) => {
    if (from === to) return;
    const key = edgeKey({ from, to, kind });
    let edge = edgesByKey.get(key);
    if (!edge) {
      edge = { from, to, kind, evidence: [] };
      edgesByKey.set(key, edge);
    }
    addEvidence(edge, file, detail);
  };

  // Declared edges — from each boundary's own package.json.
  for (const boundary of boundaries) {
    const pkg = readJson(join(boundary.dir, "package.json"));
    if (!pkg) continue;
    const deps = {
      ...(pkg.dependencies as Record<string, string> | undefined),
      ...(pkg.devDependencies as Record<string, string> | undefined),
      ...(pkg.peerDependencies as Record<string, string> | undefined),
    };
    for (const depName of Object.keys(deps)) {
      const target = byName.get(depName);
      if (!target) continue;
      upsertEdge(
        boundary.id,
        target.id,
        "declared",
        "package.json",
        `declares dependency on "${depName}"`,
      );
    }
  }

  // Import edges — scan every boundary's own files for imports of another boundary.
  for (const boundary of boundaries) {
    for (const file of boundary.files) {
      let content: string;
      try {
        content = readFileSync(join(rootDir, file), "utf8");
      } catch {
        continue;
      }
      for (const match of content.matchAll(IMPORT_SPECIFIER)) {
        const specifier = match[1];
        if (!specifier) continue;

        if (specifier.startsWith(".")) {
          // Relative import — only matters if it crosses into another boundary's directory.
          const resolved = posixNormalize(posixJoin(posixDirname(file), specifier));
          const target = resolveModuleForFile(resolved);
          if (target && target !== boundary.id) {
            upsertEdge(boundary.id, target, "import", file, `imports "${specifier}"`);
          }
          continue;
        }

        const targetName = namesByLength.find(
          (name) => specifier === name || specifier.startsWith(`${name}/`),
        );
        if (targetName) {
          const target = byName.get(targetName);
          if (target) upsertEdge(boundary.id, target.id, "import", file, `imports "${specifier}"`);
        }
      }
    }
  }

  return {
    nodes: boundaries.map((b) => b.id).sort(),
    edges: [...edgesByKey.values()].sort(
      (a, b) =>
        a.from.localeCompare(b.from) || a.to.localeCompare(b.to) || a.kind.localeCompare(b.kind),
    ),
  };
}
