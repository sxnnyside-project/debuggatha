import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";

/**
 * What Debuggatha is allowed to look at. A review of code the project does not
 * own (dependencies, build output, generated or minified files) is noise the
 * user cannot act on, so it is excluded once, here, for every review and every
 * analysis.
 */

/** Directories that hold dependencies or build output, never the project's own source. */
export const IGNORED_DIRS: ReadonlySet<string> = new Set([
  "node_modules",
  "vendor",
  "bower_components",
  "jspm_packages",
  "Pods",
  "Carthage",
  "target",
  "build",
  "dist",
  "out",
  "coverage",
  "__pycache__",
  "site-packages",
  "venv",
  "obj",
  ".gradle",
  ".dart_tool",
  ".next",
  ".nuxt",
  ".svelte-kit",
  ".turbo",
  ".venv",
]);

/** `bin` and `obj` are build output only beside a build tool; a Node project's `bin/` holds real scripts. */
const BUILD_OUTPUT_DIRS = new Set(["bin", "obj"]);
const BUILD_MARKERS = [
  "build.gradle",
  "build.gradle.kts",
  "settings.gradle",
  "settings.gradle.kts",
  "pom.xml",
  "Makefile",
  "CMakeLists.txt",
];

const GENERATED_FILE_NAME =
  /(\.min\.[a-z]+$|\.bundle\.[a-z]+$|\.generated\.[a-z]+$|\.g\.dart$|\.freezed\.dart$|\.pb\.go$|_pb2\.py$|\.d\.ts$|\.map$)/i;

const TEST_PATH =
  /(^|\/)(__tests__|tests?|specs?|__mocks__|mocks?|fixtures?|e2e|testdata|androidTest)(\/|$)|\.(test|spec)\.[a-z]+$|(Test|Tests|IT)\.(kt|java|cs|php)$|_test\.(go|py|dart)$/i;

const toPosix = (path: string) => path.split(sep).join("/");

function hasBuildMarker(dir: string): boolean {
  if (BUILD_MARKERS.some((marker) => existsSync(join(dir, marker)))) return true;
  try {
    return readdirSync(dir).some((name) => /\.(csproj|fsproj|vbproj|sln)$/i.test(name));
  } catch {
    return false;
  }
}

/**
 * True when a repository-relative path lives in a dependency, build output, or
 * hidden directory, or is named like a generated file. Without a `rootDir` (a
 * diff has none) `bin/` and `obj/` cannot be told apart from real source and
 * are kept.
 */
export function isIgnoredPath(rootDir: string | undefined, relativePath: string): boolean {
  const posix = toPosix(relativePath);
  const segments = posix.split("/");
  const fileName = segments[segments.length - 1] ?? "";
  if (GENERATED_FILE_NAME.test(fileName)) return true;

  let parent = rootDir;
  for (const segment of segments.slice(0, -1)) {
    if (segment.startsWith(".") && segment !== "." && segment !== "..") return true;
    if (IGNORED_DIRS.has(segment)) return true;
    if (parent !== undefined) {
      if (BUILD_OUTPUT_DIRS.has(segment) && hasBuildMarker(parent)) return true;
      parent = join(parent, segment);
    }
  }
  return false;
}

/** Content a person did not write: a generated-file header, or minified code. */
export function isGeneratedContent(content: string): boolean {
  const head = content.split("\n", 8).join("\n");
  if (/@generated|DO NOT EDIT|auto-?generated|this file (?:was|is) generated/i.test(head)) {
    return true;
  }
  if (content.length < 2_000) return false;
  const lines = content.split("\n");
  const longest = lines.reduce((max, line) => Math.max(max, line.length), 0);
  return longest > 1_000 || content.length / lines.length > 300;
}

/** Tests carry fake credentials and deliberately bad code by design. */
export function isTestPath(relativePath: string): boolean {
  return TEST_PATH.test(toPosix(relativePath));
}

/** Files git tracks or would track, so the repository's own `.gitignore` decides; `undefined` outside a git repository. */
function gitListedFiles(rootDir: string): string[] | undefined {
  try {
    const output = execFileSync(
      "git",
      ["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
      {
        cwd: rootDir,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
        maxBuffer: 256 * 1024 * 1024,
      },
    );
    return output.split("\0").filter(Boolean);
  } catch {
    return undefined;
  }
}

function walkFiles(rootDir: string, dir: string, into: string[]): void {
  let entries: string[];
  try {
    entries = readdirSync(dir);
  } catch {
    return;
  }
  for (const entry of entries) {
    if (entry.startsWith(".") || IGNORED_DIRS.has(entry)) continue;
    const full = join(dir, entry);
    let stats: ReturnType<typeof statSync>;
    try {
      stats = statSync(full);
    } catch {
      continue;
    }
    if (stats.isDirectory()) walkFiles(rootDir, full, into);
    else into.push(toPosix(relative(rootDir, full)));
  }
}

/**
 * Every file the project owns whose name matches `extensions`, as
 * repository-relative POSIX paths in a stable order.
 */
export function listScanFiles(rootDir: string, extensions: RegExp): string[] {
  const candidates = gitListedFiles(rootDir);
  const all: string[] = candidates ?? [];
  if (!candidates) walkFiles(rootDir, rootDir, all);
  return [...new Set(all)]
    .filter((path) => extensions.test(path) && !isIgnoredPath(rootDir, path))
    .filter((path) => {
      try {
        return statSync(join(rootDir, path)).isFile();
      } catch {
        return false;
      }
    })
    .sort();
}
