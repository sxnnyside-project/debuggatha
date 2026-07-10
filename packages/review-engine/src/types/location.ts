export interface LineRange {
  start: number;
  end: number;
}

/** A finding's occurrence: a file, and optionally a specific line range within it. */
export interface FindingLocation {
  file: string;
  lines: LineRange | undefined;
}
