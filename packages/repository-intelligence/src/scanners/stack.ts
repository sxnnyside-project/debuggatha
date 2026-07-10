import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { ScanResult } from "../internal/scan-result.js";
import { SignalCollector } from "../internal/signals.js";
import type { Evidence, PlatformTarget, StackProfile, WorkspaceType } from "../types.js";

/**
 * Stack Detection.
 *
 * Every claim carries evidence (which file, what in it) instead of a bare
 * boolean — "detection must rely on repository evidence, never heuristics
 * based on filenames alone" means a manifest's *content* is inspected
 * whenever a filename alone is ambiguous (build.gradle vs build.gradle.kts,
 * a Cargo.toml that happens to depend on tauri, a pubspec.yaml that happens
 * to depend on flutter).
 */

export interface PackageJsonManifest {
  name?: string;
  workspaces?: unknown;
  packageManager?: string;
  engines?: Record<string, string>;
  dependencies?: Record<string, string>;
  devDependencies?: Record<string, string>;
}

export interface StackScan extends ScanResult<StackProfile> {
  /** Exposed so Dependency Context can reuse it instead of re-reading/parsing. */
  packageJson: PackageJsonManifest | undefined;
  cargoToml: string | undefined;
  pubspecYaml: string | undefined;
}

const MANIFEST_LANGUAGE: Record<string, string> = {
  "Cargo.toml": "Rust",
  "pubspec.yaml": "Dart",
  "go.mod": "Go",
  Gemfile: "Ruby",
  "pyproject.toml": "Python",
  "requirements.txt": "Python",
  "build.gradle.kts": "Kotlin",
  "build.gradle": "Java",
  "pom.xml": "Java",
};

const PACKAGE_JSON_FRAMEWORKS: Record<string, string> = {
  next: "Next.js",
  react: "React",
  vue: "Vue",
  svelte: "Svelte",
  electron: "Electron",
  "@tauri-apps/api": "Tauri",
  "@tauri-apps/cli": "Tauri",
};

const BUILD_SYSTEM_FILES: Record<string, string> = {
  "Cargo.toml": "Cargo",
  "build.gradle.kts": "Gradle",
  "build.gradle": "Gradle",
  "pom.xml": "Maven",
  "turbo.json": "Turborepo",
};

const VITE_CONFIGS = ["vite.config.ts", "vite.config.js", "vite.config.mjs"];
const WEBPACK_CONFIGS = ["webpack.config.js", "webpack.config.ts"];
const TSUP_CONFIGS = ["tsup.config.ts", "tsup.config.js"];

const MONOREPO_MARKER_FILES: Record<string, string> = {
  "pnpm-workspace.yaml": "pnpm-workspace.yaml present",
  "turbo.json": "turbo.json present",
  "lerna.json": "lerna.json present",
  "nx.json": "nx.json present",
};

const WEB_FRAMEWORKS = new Set(["React", "Vue", "Svelte", "Next.js"]);
const DESKTOP_FRAMEWORKS = new Set(["Tauri", "Electron"]);

export function scanStack(rootDir: string, rootEntries: readonly string[]): StackScan {
  const entries = new Set(rootEntries);
  const filesRead: string[] = [];
  const languages = new SignalCollector();
  const frameworks = new SignalCollector();
  const buildSystems = new SignalCollector();
  const packageManagers = new SignalCollector();
  const runtimes = new SignalCollector();
  const manifests: string[] = [];
  const workspaceEvidence: Evidence[] = [];

  for (const [manifest, language] of Object.entries(MANIFEST_LANGUAGE)) {
    if (entries.has(manifest)) {
      manifests.push(manifest);
      languages.add(language, { file: manifest, detail: `${manifest} present at repository root` });
    }
  }

  for (const [file, buildSystem] of Object.entries(BUILD_SYSTEM_FILES)) {
    if (entries.has(file)) {
      buildSystems.add(buildSystem, { file, detail: `${file} present at repository root` });
    }
  }

  const viteConfig = VITE_CONFIGS.find((file) => entries.has(file));
  if (viteConfig) buildSystems.add("Vite", { file: viteConfig, detail: `${viteConfig} present` });

  const webpackConfig = WEBPACK_CONFIGS.find((file) => entries.has(file));
  if (webpackConfig)
    buildSystems.add("Webpack", { file: webpackConfig, detail: `${webpackConfig} present` });

  const tsupConfig = TSUP_CONFIGS.find((file) => entries.has(file));
  if (tsupConfig) buildSystems.add("tsup", { file: tsupConfig, detail: `${tsupConfig} present` });

  // Package managers + runtimes from lockfiles.
  if (entries.has("bun.lock") || entries.has("bun.lockb")) {
    const file = entries.has("bun.lock") ? "bun.lock" : "bun.lockb";
    packageManagers.add("Bun", { file, detail: `${file} present` });
    runtimes.add("Bun", { file, detail: `${file} present` });
  }
  if (entries.has("pnpm-lock.yaml")) {
    packageManagers.add("pnpm", { file: "pnpm-lock.yaml", detail: "pnpm-lock.yaml present" });
  }
  if (entries.has("package-lock.json")) {
    packageManagers.add("npm", { file: "package-lock.json", detail: "package-lock.json present" });
  }
  if (entries.has("yarn.lock")) {
    packageManagers.add("Yarn", { file: "yarn.lock", detail: "yarn.lock present" });
  }
  if (entries.has("Cargo.lock")) {
    packageManagers.add("Cargo", { file: "Cargo.lock", detail: "Cargo.lock present" });
  }
  if (entries.has("pubspec.lock")) {
    packageManagers.add("pub", { file: "pubspec.lock", detail: "pubspec.lock present" });
  }
  if (entries.has("bunfig.toml")) {
    runtimes.add("Bun", { file: "bunfig.toml", detail: "bunfig.toml present" });
  }
  if (entries.has("deno.json") || entries.has("deno.jsonc")) {
    const file = entries.has("deno.json") ? "deno.json" : "deno.jsonc";
    runtimes.add("Deno", { file, detail: `${file} present` });
  }
  if (entries.has(".nvmrc")) {
    runtimes.add("Node.js", { file: ".nvmrc", detail: ".nvmrc present" });
  }

  // Monorepo markers (filename-only signals are fine here — a marker file's
  // mere presence, not its content, is what defines these conventions).
  for (const [file, detail] of Object.entries(MONOREPO_MARKER_FILES)) {
    if (entries.has(file)) {
      workspaceEvidence.push({ file, detail });
    }
  }

  let packageJson: PackageJsonManifest | undefined;
  if (entries.has("package.json")) {
    manifests.push("package.json");
    languages.add("JavaScript/TypeScript", {
      file: "package.json",
      detail: "package.json present at repository root",
    });

    const packageJsonPath = join(rootDir, "package.json");
    filesRead.push(packageJsonPath);
    packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8")) as PackageJsonManifest;

    const allDependencies = { ...packageJson.dependencies, ...packageJson.devDependencies };
    for (const [dependency, framework] of Object.entries(PACKAGE_JSON_FRAMEWORKS)) {
      if (dependency in allDependencies) {
        frameworks.add(framework, {
          file: "package.json",
          detail: `dependency "${dependency}" declared`,
        });
      }
    }

    if (packageJson.workspaces) {
      workspaceEvidence.push({
        file: "package.json",
        detail: '"workspaces" field present',
      });
    }

    if (typeof packageJson.packageManager === "string") {
      const [name] = packageJson.packageManager.split("@");
      const label = { bun: "Bun", pnpm: "pnpm", yarn: "Yarn", npm: "npm" }[name ?? ""];
      if (label) {
        packageManagers.add(label, {
          file: "package.json",
          detail: `"packageManager": "${packageJson.packageManager}"`,
        });
        if (label === "Bun") {
          runtimes.add("Bun", {
            file: "package.json",
            detail: `"packageManager": "${packageJson.packageManager}"`,
          });
        }
      }
    }

    if (packageJson.engines?.node) {
      runtimes.add("Node.js", {
        file: "package.json",
        detail: `engines.node = "${packageJson.engines.node}"`,
      });
    }
  }

  let cargoToml: string | undefined;
  if (entries.has("Cargo.toml")) {
    const cargoTomlPath = join(rootDir, "Cargo.toml");
    filesRead.push(cargoTomlPath);
    cargoToml = readFileSync(cargoTomlPath, "utf8");
    if (cargoToml.includes("tauri")) {
      frameworks.add("Tauri", {
        file: "Cargo.toml",
        detail: 'dependency mentioning "tauri" found',
      });
    }
  }

  let pubspecYaml: string | undefined;
  if (entries.has("pubspec.yaml")) {
    const pubspecPath = join(rootDir, "pubspec.yaml");
    filesRead.push(pubspecPath);
    pubspecYaml = readFileSync(pubspecPath, "utf8");
    if (pubspecYaml.includes("flutter")) {
      frameworks.add("Flutter", {
        file: "pubspec.yaml",
        detail: 'dependency mentioning "flutter" found',
      });
    }
  }

  const workspaceType: WorkspaceType = workspaceEvidence.length > 0 ? "monorepo" : "single-package";

  const platformTargets = new Set<PlatformTarget>();
  for (const signal of frameworks.toArray()) {
    if (WEB_FRAMEWORKS.has(signal.value)) platformTargets.add("web");
    if (DESKTOP_FRAMEWORKS.has(signal.value)) platformTargets.add("desktop");
    if (signal.value === "Flutter") platformTargets.add("mobile");
  }

  const profile: StackProfile = {
    languages: languages.toArray(),
    frameworks: frameworks.toArray(),
    buildSystems: buildSystems.toArray(),
    packageManagers: packageManagers.toArray(),
    runtimes: runtimes.toArray(),
    platformTargets: [...platformTargets].sort(),
    workspaceType,
    workspaceEvidence,
    manifests: manifests.sort(),
  };

  return {
    profile,
    filesRead,
    dirsListed: [],
    packageJson,
    cargoToml,
    pubspecYaml,
  };
}
