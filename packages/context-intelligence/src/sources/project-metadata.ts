import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import type { ProjectMetadataItem } from "../types.js";

const LICENSE_FILES = ["LICENSE", "LICENSE.md", "LICENSE.txt"];

interface PackageJsonSubset {
  license?: string;
  private?: boolean;
  version?: string;
}

/**
 * Project Metadata (Epic 14): repository identity signals not already
 * covered by Repository Intelligence's `StackProfile`/`DependencyProfile`
 * (Epic 1) or `Capability[]` (Epic 12.5) — licensing and the declared
 * package version. Deliberately narrow: workspace type, package
 * managers, and build tooling are already `Capability`
 * (`characteristic`/`tooling`) items via `buildCapabilityItems` — this
 * module doesn't re-derive them.
 */
export function buildProjectMetadataItems(rootDir: string): ProjectMetadataItem[] {
  const items: ProjectMetadataItem[] = [];

  const licenseFile = LICENSE_FILES.find((file) => existsSync(join(rootDir, file)));
  const packageJsonPath = join(rootDir, "package.json");
  let packageJson: PackageJsonSubset | undefined;
  if (existsSync(packageJsonPath)) {
    try {
      packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"));
    } catch {
      packageJson = undefined;
    }
  }

  if (licenseFile) {
    items.push({
      id: "project-metadata:license-file",
      category: "project-metadata",
      confidence: "detected",
      evidence: [{ file: licenseFile, detail: `${licenseFile} present at repository root` }],
      source: licenseFile,
      key: "license",
      value: licenseFile,
    });
  } else if (packageJson?.license) {
    items.push({
      id: "project-metadata:license-field",
      category: "project-metadata",
      confidence: "detected",
      evidence: [{ file: "package.json", detail: `"license": "${packageJson.license}"` }],
      source: "package.json",
      key: "license",
      value: packageJson.license,
    });
  }

  if (packageJson?.version) {
    items.push({
      id: "project-metadata:version",
      category: "project-metadata",
      confidence: "detected",
      evidence: [{ file: "package.json", detail: `"version": "${packageJson.version}"` }],
      source: "package.json",
      key: "version",
      value: packageJson.version,
    });
  }

  if (packageJson?.private === true) {
    items.push({
      id: "project-metadata:private",
      category: "project-metadata",
      confidence: "detected",
      evidence: [{ file: "package.json", detail: '"private": true' }],
      source: "package.json",
      key: "distribution",
      value: "private (not published to a registry)",
    });
  }

  return items;
}
