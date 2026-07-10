import { readdirSync, unlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { withTempRepo } from "@debuggatha/testing";
import { afterEach, describe, expect, it } from "vitest";
import { buildFingerprint, hasFingerprintChanged } from "./fingerprint.js";

describe("fingerprint", () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    cleanup?.();
    cleanup = undefined;
  });

  it("reports no change for an untouched repository", () => {
    const repo = withTempRepo({ "Cargo.toml": '[package]\nname = "demo"\n' });
    cleanup = repo.cleanup;

    const filePath = join(repo.root, "Cargo.toml");
    const fingerprint = buildFingerprint(
      [{ dir: repo.root, entries: readdirSync(repo.root).sort() }],
      [filePath],
    );

    expect(hasFingerprintChanged(fingerprint)).toBe(false);
  });

  it("detects a tracked file's content changing", () => {
    const repo = withTempRepo({ "Cargo.toml": '[package]\nname = "demo"\n' });
    cleanup = repo.cleanup;

    const filePath = join(repo.root, "Cargo.toml");
    const fingerprint = buildFingerprint([], [filePath]);

    writeFileSync(filePath, '[package]\nname = "renamed"\n\n[dependencies]\ntauri = "2.0"\n');

    expect(hasFingerprintChanged(fingerprint)).toBe(true);
  });

  it("detects a new file appearing in a tracked directory, not just tracked-file mutations", () => {
    const repo = withTempRepo({ "README.md": "# Demo\n" });
    cleanup = repo.cleanup;

    const fingerprint = buildFingerprint(
      [{ dir: repo.root, entries: readdirSync(repo.root).sort() }],
      [],
    );

    writeFileSync(join(repo.root, "Cargo.toml"), '[package]\nname = "demo"\n');

    expect(hasFingerprintChanged(fingerprint)).toBe(true);
  });

  it("detects a tracked file being deleted", () => {
    const repo = withTempRepo({ "Cargo.toml": '[package]\nname = "demo"\n' });
    cleanup = repo.cleanup;

    const filePath = join(repo.root, "Cargo.toml");
    const fingerprint = buildFingerprint([], [filePath]);

    unlinkSync(filePath);

    expect(hasFingerprintChanged(fingerprint)).toBe(true);
  });
});
