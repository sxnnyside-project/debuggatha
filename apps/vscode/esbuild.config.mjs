import { build, context } from "esbuild";

const watch = process.argv.includes("--watch");
const tests = process.argv.includes("--tests");

const shared = {
  bundle: true,
  platform: "node",
  format: "cjs",
  // VS Code 1.90, the oldest supported host, runs Node 20.
  target: "node20",
  sourcemap: true,
  logLevel: "info",
};

/** @type {import("esbuild").BuildOptions} */
const extension = {
  ...shared,
  entryPoints: ["src/index.ts"],
  outfile: "dist/index.js",
  minify: !watch,
  external: ["vscode"],
};

// The process a review runs in, so a slow analyzer cannot freeze the editor and a review can be
// cancelled. It carries the engine and needs nothing from VS Code.
/** @type {import("esbuild").BuildOptions} */
const worker = {
  ...shared,
  entryPoints: ["src/runner/worker.ts"],
  outfile: "dist/review-worker.js",
  minify: !watch,
};

// Integration tests run inside a real VS Code: `run.js` launches it, `suite.js` and
// `extension.itest.js` execute in the extension host.
/** @type {import("esbuild").BuildOptions} */
const integration = {
  ...shared,
  entryPoints: [
    "src/test/integration/run.ts",
    "src/test/integration/suite.ts",
    "src/test/integration/extension.itest.ts",
  ],
  outdir: "dist-test",
  external: ["vscode", "mocha", "@vscode/test-electron"],
};

if (watch) {
  for (const options of [extension, worker]) await (await context(options)).watch();
} else if (tests) {
  await build(integration);
} else {
  await Promise.all([build(extension), build(worker)]);
}
