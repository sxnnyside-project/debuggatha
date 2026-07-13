import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import type { PackageJsonManifest, StackScan } from "./scanners/stack.js";
import type { Capability, CapabilityConfidence, CapabilityKind, Evidence } from "./types.js";

/**
 * Repository Capability Resolution (Epic 12.5). Turns the coarse,
 * display-oriented `StackProfile` signals (Epic 1) into a flat,
 * additive set of normalized capability ids — the fix for the exact
 * limitation Epic 11 surfaced: `StackProfile.languages` carries one
 * combined `"JavaScript/TypeScript"` signal, but Review Packs (and Rule
 * Resolution) need to ask "does this repo have TypeScript?" and "does it
 * have JavaScript?" as two independent, simultaneously-true-or-false
 * questions — never a single mutually-exclusive "stack" label.
 *
 * Additive by construction: a repository with TypeScript, React, Bun,
 * Turborepo, and Tauri gets five independent capabilities, not one
 * label chosen among them. Nothing here ever picks "the" language or
 * framework — every detected fact becomes its own `Capability`.
 */
function normalizeId(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9]/g, "");
}

function capability(
  id: string,
  kind: CapabilityKind,
  confidence: CapabilityConfidence,
  evidence: Evidence[],
  origin: Capability["origin"],
): Capability {
  return { id: normalizeId(id), kind, confidence, evidence, origin };
}

const EXTRA_PACKAGE_JSON_FRAMEWORKS: Record<string, string> = {
  astro: "astro",
  lit: "lit",
  express: "express",
  fastify: "fastify",
  hono: "hono",
  elysia: "elysia",
};

const TOOLING_CONFIG_FILES: Record<string, string> = {
  "biome.json": "biome",
  "biome.jsonc": "biome",
  ".eslintrc.json": "eslint",
  ".eslintrc.js": "eslint",
  ".eslintrc.cjs": "eslint",
  "eslint.config.js": "eslint",
  "eslint.config.mjs": "eslint",
  "eslint.config.ts": "eslint",
  "vitest.config.ts": "vitest",
  "vitest.config.js": "vitest",
  "vitest.config.mts": "vitest",
};

const CHARACTERISTIC_DIRS = ["packages", "apps"];
const FEATURE_DIR_NAMES = new Set(["features", "modules", "domains"]);
const LAYER_DIR_NAMES = new Set(["domain", "adapters", "infrastructure", "application"]);

function detectTypeScript(
  rootEntries: readonly string[],
  packageJson: PackageJsonManifest | undefined,
): Capability | undefined {
  const evidence: Evidence[] = [];
  let confidence: CapabilityConfidence = "low";

  if (rootEntries.includes("tsconfig.json")) {
    evidence.push({ file: "tsconfig.json", detail: "tsconfig.json present at repository root" });
    confidence = "high";
  }
  const deps = { ...packageJson?.dependencies, ...packageJson?.devDependencies };
  if ("typescript" in deps) {
    evidence.push({ file: "package.json", detail: '"typescript" declared as a dependency' });
    confidence = confidence === "high" ? "high" : "medium";
  }

  if (evidence.length === 0) return undefined;
  return capability("typescript", "language", confidence, evidence, "manifest");
}

function detectVscodeExtension(
  packageJson: PackageJsonManifest | undefined,
): Capability | undefined {
  const engines = packageJson?.engines as Record<string, string> | undefined;
  if (!engines?.vscode) return undefined;
  return capability(
    "vscode-extension",
    "platform",
    "high",
    [{ file: "package.json", detail: `engines.vscode = "${engines.vscode}"` }],
    "manifest",
  );
}

function detectPhpLaravel(rootDir: string, rootEntries: readonly string[]): Capability[] {
  if (!rootEntries.includes("composer.json")) return [];
  const capabilities: Capability[] = [
    capability(
      "php",
      "language",
      "high",
      [{ file: "composer.json", detail: "composer.json present at repository root" }],
      "manifest",
    ),
  ];
  try {
    const composer = JSON.parse(readFileSync(join(rootDir, "composer.json"), "utf8")) as {
      require?: Record<string, string>;
    };
    if (composer.require && "laravel/framework" in composer.require) {
      capabilities.push(
        capability(
          "laravel",
          "framework",
          "high",
          [{ file: "composer.json", detail: '"laravel/framework" declared as a dependency' }],
          "manifest",
        ),
      );
    }
  } catch {
    // composer.json present but unparseable — php capability alone still stands on file presence
  }
  return capabilities;
}

function detectCharacteristics(rootDir: string, rootEntries: readonly string[]): Capability[] {
  const capabilities: Capability[] = [];

  const workspaceDirs = CHARACTERISTIC_DIRS.filter((dir) => rootEntries.includes(dir));
  if (workspaceDirs.length > 0) {
    capabilities.push(
      capability(
        "monorepo",
        "characteristic",
        "high",
        workspaceDirs.map((dir) => ({
          file: dir,
          detail: `"${dir}" directory present at repository root`,
        })),
        "convention",
      ),
    );
    capabilities.push(
      capability(
        "package-based",
        "characteristic",
        "high",
        workspaceDirs.map((dir) => ({
          file: dir,
          detail: `"${dir}" directory present at repository root`,
        })),
        "convention",
      ),
    );
  }
  if (rootEntries.includes("pnpm-workspace.yaml") || rootEntries.includes("turbo.json")) {
    capabilities.push(
      capability(
        "workspace",
        "characteristic",
        "high",
        [
          rootEntries.includes("pnpm-workspace.yaml")
            ? { file: "pnpm-workspace.yaml", detail: "pnpm-workspace.yaml present" }
            : { file: "turbo.json", detail: "turbo.json present" },
        ],
        "convention",
      ),
    );
  }

  const srcDir = join(rootDir, "src");
  if (existsSync(srcDir)) {
    let srcEntries: string[] = [];
    try {
      srcEntries = readdirSync(srcDir);
    } catch {
      srcEntries = [];
    }
    const hasFeatureDir = srcEntries.some((entry) => FEATURE_DIR_NAMES.has(entry));
    const layerDirsPresent = srcEntries.filter((entry) => LAYER_DIR_NAMES.has(entry));

    if (hasFeatureDir) {
      capabilities.push(
        capability(
          "feature-based",
          "characteristic",
          "medium",
          srcEntries
            .filter((entry) => FEATURE_DIR_NAMES.has(entry))
            .map((entry) => ({ file: `src/${entry}`, detail: `src/${entry} directory present` })),
          "convention",
        ),
      );
    }
    if (layerDirsPresent.length >= 2) {
      capabilities.push(
        capability(
          "layered",
          "characteristic",
          "medium",
          layerDirsPresent.map((entry) => ({
            file: `src/${entry}`,
            detail: `src/${entry} directory present`,
          })),
          "convention",
        ),
      );
    }
  }

  return capabilities;
}

/**
 * Derives the flat, additive `Capability[]` for a repository. Reuses
 * `StackScan`'s already-parsed manifests (`packageJson`/`cargoToml`/
 * `pubspecYaml`) instead of re-reading them — this is the same scan
 * Stack Detection already ran, not a second traversal.
 */
export function deriveCapabilities(
  rootDir: string,
  rootEntries: readonly string[],
  stackScan: StackScan,
): Capability[] {
  const capabilities: Capability[] = [];
  const { profile, packageJson } = stackScan;

  for (const signal of profile.languages) {
    if (signal.value === "JavaScript/TypeScript") {
      capabilities.push(capability("javascript", "language", "high", signal.evidence, "manifest"));
      const typescript = detectTypeScript(rootEntries, packageJson);
      if (typescript) capabilities.push(typescript);
      continue;
    }
    capabilities.push(capability(signal.value, "language", "high", signal.evidence, "manifest"));
  }

  for (const signal of profile.frameworks) {
    capabilities.push(capability(signal.value, "framework", "high", signal.evidence, "manifest"));
  }

  const deps = { ...packageJson?.dependencies, ...packageJson?.devDependencies };
  for (const [dependency, id] of Object.entries(EXTRA_PACKAGE_JSON_FRAMEWORKS)) {
    if (dependency in deps) {
      capabilities.push(
        capability(
          id,
          "framework",
          "high",
          [{ file: "package.json", detail: `dependency "${dependency}" declared` }],
          "manifest",
        ),
      );
    }
  }

  for (const target of profile.platformTargets) {
    capabilities.push(
      capability(
        target,
        "platform",
        "high",
        [
          {
            file: "package.json",
            detail: `platform target "${target}" inferred from detected frameworks`,
          },
        ],
        "convention",
      ),
    );
  }
  const vscodeExtension = detectVscodeExtension(packageJson);
  if (vscodeExtension) capabilities.push(vscodeExtension);

  for (const signal of profile.runtimes) {
    capabilities.push(capability(signal.value, "tooling", "high", signal.evidence, "manifest"));
  }
  for (const signal of profile.packageManagers) {
    capabilities.push(capability(signal.value, "tooling", "high", signal.evidence, "manifest"));
  }
  for (const signal of profile.buildSystems) {
    capabilities.push(capability(signal.value, "tooling", "high", signal.evidence, "manifest"));
  }
  for (const [file, id] of Object.entries(TOOLING_CONFIG_FILES)) {
    if (rootEntries.includes(file)) {
      capabilities.push(
        capability(
          id,
          "tooling",
          "high",
          [{ file, detail: `${file} present at repository root` }],
          "config",
        ),
      );
    }
  }

  capabilities.push(...detectPhpLaravel(rootDir, rootEntries));
  capabilities.push(...detectCharacteristics(rootDir, rootEntries));

  // Dedupe by id — multiple signals can independently point at the same
  // capability (e.g. "bun" from both a lockfile and bunfig.toml); keep
  // the first occurrence's confidence but merge evidence.
  const byId = new Map<string, Capability>();
  for (const cap of capabilities) {
    const existing = byId.get(cap.id);
    if (!existing) {
      byId.set(cap.id, cap);
      continue;
    }
    byId.set(cap.id, { ...existing, evidence: [...existing.evidence, ...cap.evidence] });
  }

  return [...byId.values()].sort(
    (a, b) => a.kind.localeCompare(b.kind) || a.id.localeCompare(b.id),
  );
}
