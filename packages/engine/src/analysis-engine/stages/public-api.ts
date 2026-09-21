import { readFileSync } from "node:fs";
import { join } from "node:path";
import { readJson } from "../internal/fs-walk.js";
import type { ModuleBoundary, PublicApiSurface, PublicApiSymbol } from "../types.js";

const EXPORT_DECLARATION =
  /^export\s+(?:declare\s+)?(?:async\s+)?(?:function|const|class|interface|type|enum)\s+([A-Za-z_$][\w$]*)/gm;
const RE_EXPORT = /^export\s*\{([^}]*)\}\s*from/gm;

function entrypointFor(rootDir: string, boundary: ModuleBoundary): string | undefined {
  const pkg = readJson(join(boundary.dir, "package.json"));
  const main =
    typeof pkg?.module === "string"
      ? pkg.module
      : typeof pkg?.main === "string"
        ? pkg.main
        : undefined;
  const candidates = [
    main ? join(boundary.dir, main).replace(/\.js$/, ".ts") : undefined,
    join(boundary.dir, "src", "index.ts"),
    join(boundary.dir, "index.ts"),
  ].filter((c): c is string => c !== undefined);

  for (const candidate of candidates) {
    const relative = candidate.replace(`${rootDir}/`, "");
    if (boundary.files.includes(relative)) return relative;
  }
  return undefined;
}

function scanExports(rootDir: string, file: string): PublicApiSymbol[] {
  let content: string;
  try {
    content = readFileSync(join(rootDir, file), "utf8");
  } catch {
    return [];
  }
  const symbols: PublicApiSymbol[] = [];
  const lines = content.split("\n");

  lines.forEach((line, index) => {
    EXPORT_DECLARATION.lastIndex = 0;
    const declMatch = EXPORT_DECLARATION.exec(line);
    if (declMatch?.[1]) {
      symbols.push({
        name: declMatch[1],
        file,
        line: index + 1,
        stability: declMatch[1].startsWith("_") ? "unstable" : "stable",
      });
    }
    RE_EXPORT.lastIndex = 0;
    const reExportMatch = RE_EXPORT.exec(line);
    if (reExportMatch?.[1]) {
      for (const name of reExportMatch[1].split(",")) {
        const trimmed = name
          .replace(/^type\s+/, "")
          .split(/\s+as\s+/)
          .pop()
          ?.trim();
        if (trimmed) symbols.push({ name: trimmed, file, line: index + 1, stability: "stable" });
      }
    }
  });

  return symbols;
}

/**
 * Public API Surface. Symbols exported from a module's own
 * entrypoint (`package.json` `main`/`module`, or `src/index.ts`/
 * `index.ts` — whichever is actually one of the module's own scanned
 * `files`) are the module's real public API. Symbols exported from any
 * *other* file the module owns are "unexpectedly exposed internals" —
 * accessible to importers today, but not the module's declared surface.
 */
export function buildPublicApiSurface(rootDir: string, boundary: ModuleBoundary): PublicApiSurface {
  const entrypoint = entrypointFor(rootDir, boundary);
  const exported: PublicApiSymbol[] = [];
  const unexpectedlyExposed: PublicApiSymbol[] = [];

  for (const file of boundary.files) {
    const symbols = scanExports(rootDir, file);
    if (symbols.length === 0) continue;
    if (file === entrypoint) exported.push(...symbols);
    else unexpectedlyExposed.push(...symbols);
  }

  return { moduleId: boundary.id, entrypoint, exported, unexpectedlyExposed };
}
