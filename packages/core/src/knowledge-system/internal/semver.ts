/**
 * Minimal `major.minor.patch` comparator — deliberately not a full semver
 * implementation (no prerelease/build metadata, no `npm semver` package
 * dependency; the epic's "no new dependency needed" applies to hashing
 * explicitly, and this package follows the same minimal-dependency
 * discipline repository-intelligence already set for config parsing).
 * Covers what v1's static, in-repo pack catalog actually needs: comparing
 * `PackDependency.versionRange` against a registered `ReviewPack.version`.
 */

interface ParsedVersion {
  major: number;
  minor: number;
  patch: number;
}

function parseVersion(version: string): ParsedVersion | undefined {
  const match = /^(\d+)\.(\d+)\.(\d+)/.exec(version.trim());
  if (!match) return undefined;
  return { major: Number(match[1]), minor: Number(match[2]), patch: Number(match[3]) };
}

/** Positive if `a` > `b`, negative if `a` < `b`, zero if equal. Unparsable versions sort last. */
export function compareVersions(a: string, b: string): number {
  const parsedA = parseVersion(a);
  const parsedB = parseVersion(b);
  if (!parsedA && !parsedB) return 0;
  if (!parsedA) return -1;
  if (!parsedB) return 1;
  if (parsedA.major !== parsedB.major) return parsedA.major - parsedB.major;
  if (parsedA.minor !== parsedB.minor) return parsedA.minor - parsedB.minor;
  return parsedA.patch - parsedB.patch;
}

/**
 * Supports the subset a static in-repo catalog needs: `*` (any), an exact
 * `x.y.z` match, `^x.y.z` (same major, >= x.y.z), `~x.y.z` (same
 * major.minor, >= x.y.z).
 */
export function satisfiesRange(version: string, range: string): boolean {
  const trimmed = range.trim();
  if (trimmed === "*" || trimmed === "") return true;

  const parsedVersion = parseVersion(version);
  if (!parsedVersion) return false;

  if (trimmed.startsWith("^")) {
    const base = parseVersion(trimmed.slice(1));
    if (!base) return false;
    return parsedVersion.major === base.major && compareVersions(version, trimmed.slice(1)) >= 0;
  }

  if (trimmed.startsWith("~")) {
    const base = parseVersion(trimmed.slice(1));
    if (!base) return false;
    return (
      parsedVersion.major === base.major &&
      parsedVersion.minor === base.minor &&
      compareVersions(version, trimmed.slice(1)) >= 0
    );
  }

  const base = parseVersion(trimmed);
  if (!base) return false;
  return compareVersions(version, trimmed) === 0;
}
