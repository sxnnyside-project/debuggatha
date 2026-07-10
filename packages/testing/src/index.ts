import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

/**
 * Materializes a fixture repository under a temp directory so skills
 * that read the filesystem (Stack Detection, Criteria Resolution, ...)
 * can be tested against real files instead of mocked fs calls.
 *
 * `files` maps relative paths to their contents, e.g.
 *   { "Cargo.toml": "[package]\nname = \"demo\"" }
 */
export function withTempRepo(files: Record<string, string>): {
  root: string;
  cleanup: () => void;
} {
  const root = mkdtempSync(join(tmpdir(), "debuggatha-"));

  for (const [relativePath, contents] of Object.entries(files)) {
    const fullPath = join(root, relativePath);
    mkdirSync(dirname(fullPath), { recursive: true });
    writeFileSync(fullPath, contents, "utf8");
  }

  return {
    root,
    cleanup: () => rmSync(root, { recursive: true, force: true }),
  };
}
