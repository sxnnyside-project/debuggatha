export interface LineRange {
  start: number;
  end: number;
}

/** A finding's occurrence: a file, and optionally a specific line range and the columns it spans on it. */
export interface FindingLocation {
  file: string;
  lines: LineRange | undefined;
  /** 1-based columns of the match on its first line (`start`) and just past its end (`end`), when the detector knows them. */
  columns?: LineRange;
}
