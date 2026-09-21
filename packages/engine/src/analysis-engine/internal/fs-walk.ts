import { readFileSync } from "node:fs";

// What the analysis may look at is decided once, for every review, in `scan-scope`.
export { IGNORED_DIRS } from "../../scan-scope/index.js";

export const SOURCE_EXTENSIONS = /\.(ts|tsx|js|jsx|mjs|cjs)$/;

export function readJson(path: string): Record<string, unknown> | undefined {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return undefined;
  }
}
