#!/usr/bin/env node
// License gate. Debuggatha runs external analyzers as separate processes and never
// includes them, so this checks the three ways that could stop being true:
//   1. every third-party package that ships inside a published artifact is under a
//      permissive license (nothing copyleft or source-available is redistributed);
//   2. each analyzer declares its own license and homepage, and none is a tool or
//      ruleset Debuggatha must not depend on (Semgrep registry rules, CodeQL);
//   3. no published package lists an analyzer as a dependency, and no built bundle
//      contains an analyzer binary.
// Run after the packages are built: `just licenses`.

import { existsSync, readdirSync, readFileSync, realpathSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const root = resolve(import.meta.dirname, "..");
const failures = [];
const fail = (message) => failures.push(message);

const read = (path) => JSON.parse(readFileSync(path, "utf8"));

/* ---------------------------------------------------------------- 1. dependencies */

const ALLOWED = new Set([
  "MIT",
  "ISC",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "Apache-2.0",
  "0BSD",
  "BlueOak-1.0.0",
  "CC0-1.0",
  "Unlicense",
  "Python-2.0",
]);

/** The licenses a package.json declares, in any of the shapes npm has allowed. */
function declaredLicense(pkg) {
  if (typeof pkg.license === "string") return pkg.license;
  if (pkg.license?.type) return pkg.license.type;
  if (Array.isArray(pkg.licenses)) {
    return pkg.licenses.map((entry) => entry.type ?? entry).join(" OR ");
  }
  return undefined;
}

/** Whether an SPDX expression can be satisfied with allowed licenses only. */
function permitted(expression) {
  const flat = expression.replace(/[()]/g, " ").trim();
  return flat
    .split(/\s+OR\s+/i)
    .some((alternative) =>
      alternative
        .split(/\s+AND\s+/i)
        .every((license) => ALLOWED.has(license.trim().replace(/^SEE LICENSE.*$/i, ""))),
    );
}

function findPackage(name, fromDir) {
  for (let dir = fromDir; ; dir = dirname(dir)) {
    const candidate = join(dir, "node_modules", name, "package.json");
    if (existsSync(candidate)) return candidate;
    if (dirname(dir) === dir) return undefined;
  }
}

/** The production dependency closure of a package, as installed. */
function closure(packageDir) {
  const seen = new Map();
  const visit = (name, fromDir, optional) => {
    const manifest = findPackage(name, fromDir);
    if (!manifest) {
      if (!optional)
        fail(`${name} is a dependency of ${fromDir} but is not installed; run \`just install\`.`);
      return;
    }
    const real = realpathSync(manifest);
    if (seen.has(real)) return;
    const pkg = read(real);
    seen.set(real, pkg);
    for (const dep of Object.keys(pkg.dependencies ?? {})) visit(dep, dirname(real), false);
    for (const dep of Object.keys(pkg.optionalDependencies ?? {})) visit(dep, dirname(real), true);
  };
  const own = read(join(packageDir, "package.json"));
  for (const dep of Object.keys(own.dependencies ?? {})) visit(dep, packageDir, false);
  return [...seen.values()];
}

const PUBLISHED = ["packages/cli", "packages/mcp", "apps/vscode"];
let scanned = 0;
for (const relative of PUBLISHED) {
  const dir = join(root, relative);
  if (!existsSync(join(dir, "package.json"))) continue;
  for (const pkg of closure(dir)) {
    scanned += 1;
    const license = declaredLicense(pkg);
    if (!license) fail(`${pkg.name}@${pkg.version} (in ${relative}) declares no license.`);
    else if (!permitted(license)) {
      fail(
        `${pkg.name}@${pkg.version} (in ${relative}) is under "${license}", which is not on the allowed list.`,
      );
    }
  }
}

/* ------------------------------------------------------------------ 2. analyzers */

const NEVER = new Map([
  ["semgrep", "its registry rules are not licensed for redistribution or commercial use"],
  ["codeql", "its queries are not licensed for use outside open source"],
]);

const enginePath = join(root, "packages/engine/dist/index.js");
if (!existsSync(enginePath)) {
  fail("packages/engine/dist is missing; build the packages first (`just licenses` does).");
}

const analyzers = [];
if (existsSync(enginePath)) {
  const { ADAPTERS } = await import(pathToFileURL(enginePath).href);
  for (const adapter of ADAPTERS) {
    analyzers.push(adapter);
    if (!adapter.license || typeof adapter.license !== "string") {
      fail(`Analyzer "${adapter.id}" declares no license; findings must show the tool's license.`);
    }
    if (!/^https:\/\//.test(adapter.homepage ?? "")) {
      fail(`Analyzer "${adapter.id}" needs an https homepage, where its license and source are.`);
    }
    if (NEVER.has(adapter.id))
      fail(`Analyzer "${adapter.id}" must not be used: ${NEVER.get(adapter.id)}.`);
  }
}

/* ---------------------------------------------------------- 3. nothing is bundled */

const analyzerPackages = new Set([
  ...analyzers.map((adapter) => adapter.id),
  "@biomejs/biome",
  "biome",
  "eslint",
  "ruff",
  "gitleaks",
  "ktlint",
  "detekt",
  "phpstan",
  "osv-scanner",
  "semgrep",
]);

for (const relative of ["packages/cli", "packages/mcp", "packages/engine", "apps/vscode"]) {
  const manifest = join(root, relative, "package.json");
  if (!existsSync(manifest)) continue;
  const pkg = read(manifest);
  for (const field of ["dependencies", "optionalDependencies", "peerDependencies"]) {
    for (const name of Object.keys(pkg[field] ?? {})) {
      if (analyzerPackages.has(name)) {
        fail(
          `${relative} lists the analyzer "${name}" in ${field}; analyzers are run, never shipped.`,
        );
      }
    }
  }
}

const BINARY = /\.(exe|dll|so|dylib|jar|phar|wasm|node)$/i;
function walk(dir, into) {
  if (!existsSync(dir)) return;
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) walk(full, into);
    else into.push(full);
  }
}
for (const relative of ["packages/cli/dist", "packages/mcp/dist"]) {
  const files = [];
  walk(join(root, relative), files);
  for (const file of files.filter((path) => BINARY.test(path))) {
    fail(
      `${relative} contains a binary (${file.slice(root.length + 1)}); a published bundle ships no analyzer.`,
    );
  }
}

/* ------------------------------------------------------------------------ report */

console.log(`Checked the licenses of ${scanned} packages that ship in published artifacts.`);
console.log("Analyzers (run as separate processes, never included):");
for (const adapter of analyzers) {
  console.log(`  ${adapter.id.padEnd(12)} ${adapter.license.padEnd(18)} ${adapter.homepage}`);
}
if (failures.length > 0) {
  console.error(`\n${failures.length} license problem${failures.length === 1 ? "" : "s"}:`);
  for (const message of failures) console.error(`  - ${message}`);
  process.exit(1);
}
console.log("\nLicense gate passed.");
