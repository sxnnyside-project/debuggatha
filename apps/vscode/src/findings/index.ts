import { isActiveFindingStatus, type LedgerEntry, type Severity } from "@debuggatha/engine";
import { SEVERITY_LEVELS } from "../config/settings.js";
import { absolutePath, type Span, spanOf } from "./present.js";

export interface IndexedFinding {
  entry: LedgerEntry;
  /** Absolute path, as the editor names files. */
  file: string;
  span: Span;
}

const rank = (severity: Severity) => SEVERITY_LEVELS.indexOf(severity);

/**
 * The ledger as the editor needs it: findings by file for squiggles, hovers, and quick fixes.
 * The ledger stays the source of truth; this is rebuilt from it whenever it changes.
 */
export class FindingIndex {
  private byFile = new Map<string, IndexedFinding[]>();
  private byId = new Map<string, LedgerEntry>();

  /** Only findings still open, and at or above `minimum`, are shown in the editor. */
  rebuild(entries: readonly LedgerEntry[], rootDir: string, minimum: Severity): void {
    this.byFile.clear();
    this.byId.clear();
    for (const entry of entries) {
      this.byId.set(entry.id, entry);
      if (!isActiveFindingStatus(entry.status)) continue;
      if (rank(entry.latestFinding.severity) < rank(minimum)) continue;
      const file = absolutePath(rootDir, entry.fingerprint.file);
      const list = this.byFile.get(file) ?? [];
      list.push({ entry, file, span: spanOf(entry.latestFinding) });
      this.byFile.set(file, list);
    }
  }

  files(): IterableIterator<string> {
    return this.byFile.keys();
  }

  inFile(file: string): readonly IndexedFinding[] {
    return this.byFile.get(file) ?? [];
  }

  /** The findings on a 0-based line. */
  onLine(file: string, line: number): IndexedFinding[] {
    return this.inFile(file).filter((finding) => finding.span.line === line);
  }

  entry(id: string): LedgerEntry | undefined {
    return this.byId.get(id);
  }
}
