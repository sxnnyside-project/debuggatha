import { describe, expect, it } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const binSource = resolve(import.meta.dir, "bin.ts");

function runBin(entry: string) {
  return Bun.spawnSync(["bun", entry, "--version"], { stdout: "pipe", stderr: "pipe" });
}

describe("debuggatha bin", () => {
  it("prints its version when run directly", () => {
    const result = runBin(binSource);
    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString().trim()).toMatch(/^\d+\.\d+\.\d+$/);
  });

  it("still runs through a symlink in a path containing spaces, the way an installed bin is invoked", () => {
    const dir = mkdtempSync(join(tmpdir(), "debuggatha bin "));
    try {
      mkdirSync(join(dir, "node modules"));
      const link = join(dir, "node modules", "debuggatha.ts");
      symlinkSync(binSource, link);
      const result = runBin(link);
      expect(result.exitCode).toBe(0);
      expect(result.stdout.toString().trim()).toMatch(/^\d+\.\d+\.\d+$/);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
