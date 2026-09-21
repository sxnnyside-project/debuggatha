import type { ScanResult } from "../internal/scan-result.js";
import type { DependencyProfile, Evidence } from "../types.js";
import type { PackageJsonManifest } from "./stack.js";

/**
 * Dependency Context.
 *
 * Repository metadata only — "do not attempt a security audit" (see the
 * epic). This surfaces what's *declared*, not what's actually installed
 * or whether it's vulnerable: lockfile presence, and version constraints
 * for the frameworks/runtimes Stack Detection already recognized. It does
 * not parse lockfile contents (see README "risks" for why).
 */

const LOCKFILES: Record<string, string> = {
  "bun.lock": "bun.lock present",
  "bun.lockb": "bun.lockb present",
  "pnpm-lock.yaml": "pnpm-lock.yaml present",
  "package-lock.json": "package-lock.json present",
  "yarn.lock": "yarn.lock present",
  "Cargo.lock": "Cargo.lock present",
  "pubspec.lock": "pubspec.lock present",
};

/** Best-effort single-line `key = "value"` extraction — not a TOML parser. */
function extractTomlStringValue(text: string, key: string): string | undefined {
  const match = text.match(new RegExp(`^\\s*${key}\\s*=\\s*"([^"]+)"`, "m"));
  return match?.[1];
}

const TRACKED_PACKAGES = [
  "react",
  "next",
  "vue",
  "svelte",
  "electron",
  "@tauri-apps/api",
  "@tauri-apps/cli",
];

export function scanDependencies(
  rootEntries: readonly string[],
  stack: { packageJson: PackageJsonManifest | undefined; cargoToml: string | undefined },
): ScanResult<DependencyProfile> {
  const entries = new Set(rootEntries);
  const lockfiles: Evidence[] = [];
  const declaredVersions: Record<string, string> = {};
  const runtimeConstraints: Record<string, string> = {};

  for (const [file, detail] of Object.entries(LOCKFILES)) {
    if (entries.has(file)) {
      lockfiles.push({ file, detail });
    }
  }

  if (stack.packageJson) {
    const all = { ...stack.packageJson.dependencies, ...stack.packageJson.devDependencies };
    for (const name of TRACKED_PACKAGES) {
      const version = all[name];
      if (version) declaredVersions[name] = version;
    }
    if (stack.packageJson.engines?.node) {
      runtimeConstraints.node = stack.packageJson.engines.node;
    }
    if (typeof stack.packageJson.packageManager === "string") {
      runtimeConstraints.packageManager = stack.packageJson.packageManager;
    }
  }

  if (stack.cargoToml) {
    const tauriVersion = extractTomlStringValue(stack.cargoToml, "tauri");
    if (tauriVersion) declaredVersions.tauri = tauriVersion;
    const rustVersion = extractTomlStringValue(stack.cargoToml, "rust-version");
    if (rustVersion) runtimeConstraints.rust = rustVersion;
  }

  const profile: DependencyProfile = { lockfiles, declaredVersions, runtimeConstraints };

  return { profile, filesRead: [], dirsListed: [] };
}
