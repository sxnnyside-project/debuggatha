import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createEmptyLedger, type Ledger } from "../types/ledger.js";
import { deserializeLedger, serializeLedger } from "./serialize.js";

/** Follows the Debuggatha convention: `.debuggatha/` inside the repository (epic §8). */
export function ledgerFilePath(repositoryRoot: string): string {
  return join(repositoryRoot, ".debuggatha", "ledger.json");
}

/** A repository with no ledger yet is not an error — it has an empty history. */
export function loadLedger(repositoryRoot: string): Ledger {
  const path = ledgerFilePath(repositoryRoot);
  if (!existsSync(path)) {
    return createEmptyLedger(repositoryRoot);
  }
  return deserializeLedger(readFileSync(path, "utf8"));
}

export function saveLedger(ledger: Ledger): void {
  const path = ledgerFilePath(ledger.repositoryRoot);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, serializeLedger(ledger), "utf8");
}
