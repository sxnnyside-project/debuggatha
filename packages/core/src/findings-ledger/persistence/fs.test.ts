import { afterEach, describe, expect, it } from "bun:test";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { withTempRepo } from "../../testing/index.js";
import { createEmptyLedger } from "../types/ledger.js";
import { ledgerFilePath, loadLedger, saveLedger } from "./fs.js";

describe("loadLedger / saveLedger", () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    cleanup?.();
    cleanup = undefined;
  });

  it("returns an empty ledger for a repository with no persisted ledger yet", () => {
    const repo = withTempRepo({ "README.md": "# Demo\n" });
    cleanup = repo.cleanup;

    const ledger = loadLedger(repo.root);

    expect(ledger.entries).toEqual([]);
    expect(existsSync(ledgerFilePath(repo.root))).toBe(false);
  });

  it("writes to .debuggatha/ledger.json and round-trips through save/load", () => {
    const repo = withTempRepo({ "README.md": "# Demo\n" });
    cleanup = repo.cleanup;
    const ledger = createEmptyLedger(repo.root);

    saveLedger(ledger);

    expect(existsSync(join(repo.root, ".debuggatha", "ledger.json"))).toBe(true);
    expect(loadLedger(repo.root)).toEqual(ledger);
  });

  it("creates the .debuggatha directory if it doesn't exist yet", () => {
    const repo = withTempRepo({ "README.md": "# Demo\n" });
    cleanup = repo.cleanup;

    expect(existsSync(join(repo.root, ".debuggatha"))).toBe(false);
    saveLedger(createEmptyLedger(repo.root));
    expect(existsSync(join(repo.root, ".debuggatha"))).toBe(true);
  });

  it("replaces the file in one step and leaves nothing beside it", () => {
    const repo = withTempRepo({ "README.md": "# Demo\n" });
    cleanup = repo.cleanup;
    saveLedger(createEmptyLedger(repo.root));
    saveLedger(createEmptyLedger(repo.root));

    expect(readdirSync(join(repo.root, ".debuggatha"))).toEqual(["ledger.json"]);
    expect(() => JSON.parse(readFileSync(ledgerFilePath(repo.root), "utf8"))).not.toThrow();
  });
});
