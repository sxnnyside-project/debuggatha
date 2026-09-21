import { realpathSync } from "node:fs";
import { isAbsolute, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const real = (path: string): string => {
  try {
    return realpathSync.native(path);
  } catch {
    return path;
  }
};

/**
 * A path or `file://` URI as a tool printed it, made repository-relative with
 * POSIX separators. A path outside the repository has nothing to attach to, so
 * it comes back `undefined`.
 */
export function toRepositoryPath(rootDir: string, raw: string): string | undefined {
  let path = raw;
  if (path.startsWith("file:")) {
    try {
      path = fileURLToPath(path);
    } catch {
      return undefined;
    }
  }
  if (!isAbsolute(path)) {
    const normalized = path.split(sep).join("/").replace(/^\.\//, "");
    return normalized.startsWith("../") ? undefined : normalized;
  }
  const relativePath = relative(real(rootDir), real(path));
  if (relativePath === "" || relativePath.startsWith("..") || isAbsolute(relativePath)) {
    return undefined;
  }
  return relativePath.split(sep).join("/");
}
