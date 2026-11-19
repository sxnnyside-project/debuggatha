import { build, context } from 'esbuild';

const watch = process.argv.includes('--watch');
const tests = process.argv.includes('--tests');

const shared = {
  bundle: true,
  platform: 'node',
  format: 'cjs',
  // VS Code 1.90, the oldest supported host, runs Node 20.
  target: 'node20',
  sourcemap: true,
  logLevel: 'info',
};

/** @type {import("esbuild").BuildOptions} */
const extension = {
  ...shared,
  entryPoints: ['src/index.ts'],
  outfile: 'dist/index.js',
  minify: !watch,
  external: ['vscode'],
};

// Integration tests run inside a real VS Code: `run.js` launches it, `suite.js` and
// `extension.itest.js` execute in the extension host.
/** @type {import("esbuild").BuildOptions} */
const integration = {
  ...shared,
  entryPoints: [
    'src/test/integration/run.ts',
    'src/test/integration/suite.ts',
    'src/test/integration/extension.itest.ts',
  ],
  outdir: 'dist-test',
  external: ['vscode', 'mocha', '@vscode/test-electron'],
};

if (watch) {
  await (await context(extension)).watch();
} else if (tests) {
  await build(integration);
} else {
  await build(extension);
}
