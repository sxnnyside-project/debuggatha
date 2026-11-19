// Writes the differential fixtures for the Rust repository intelligence by running the TypeScript
// scanners (the parity reference, ADR-0006) over small repositories that this script builds on
// disk. Run: bun scripts/parity/gen-intelligence-fixtures.ts
//
// Each case is a file tree. The Rust test builds the same tree in a temporary directory and must
// produce the same context (the root path and the build time are normalized).
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { buildRepositoryContext, summarizeCapabilities } from '../../packages/core/src/index.js';

type Entry = string | { symlink: string };
type Tree = Record<string, Entry>;

const json = (value: unknown) => JSON.stringify(value, null, 2);
const md = (lines: string[]) => lines.join('\n');

const cases: { name: string; tree: Tree }[] = [
  { name: 'empty', tree: {} },
  {
    name: 'ts-monorepo',
    tree: {
      'package.json': json({
        name: 'x',
        workspaces: ['packages/*'],
        packageManager: 'bun@1.4.0',
        engines: { node: '>=20', vscode: '^1.90.0' },
        dependencies: { react: '^18.3.0', next: '14.0.0', express: '4' },
        devDependencies: {
          typescript: '5.6.0',
          vue: '3',
          lit: '3',
          hono: '4',
          '@tauri-apps/cli': '2',
        },
      }),
      'tsconfig.json': '{}',
      'bun.lock': '',
      'bunfig.toml': '',
      'turbo.json': '{}',
      'vite.config.ts': '',
      'tsup.config.js': '',
      'biome.json': json({
        linter: {
          rules: {
            recommended: true,
            style: { noVar: 'error', useConst: { level: 'warn' } },
            preset: 'x',
          },
        },
        formatter: { enabled: true, indentStyle: 'space', lineWidth: 100 },
      }),
      'vitest.config.ts': '',
      '.editorconfig': md([
        'root = true',
        '',
        '[*]',
        'indent_style = space',
        '; comment',
        '[*.md]',
        'trim_trailing_whitespace = false',
      ]),
      'README.md': md(['# Project  ', '', 'Some words here, and more.', '']),
      'CLAUDE.md': md([
        '# Guide',
        '## Rules',
        '### Sub rule',
        '#### too deep',
        '## Another one  ',
        '##',
        '## ',
      ]),
      'ARCHITECTURE.md': md(['# Arch', '## Layers']),
      'packages/a/package.json': '{}',
      'apps/web/x.txt': '',
      'src/features/a.ts': '',
      'src/domain/a.ts': '',
      'src/adapters/a.ts': '',
      'docs/adr/0001-first.md': '# First',
      'docs/adr/0002_second.MD': '# Second',
      'docs/PRD.md': '# Product',
      'docs/roadmap.md': '# Road',
      'docs/guide/architecture-notes.md': '# Notes',
      'docs/guide/deep/a/b/c/too-deep.md': '# no',
      'docs/guide/deep/a/b/ok.md': '# yes',
      'docs/node_modules/x.md': '# ignored',
      'docs/notes.txt': 'not markdown',
      'docs/éclair.md': '# Éclair',
      'docs/日本語.md': '# 日本語',
      'docs/Zebra.md': '# Z',
      'docs/apple.md': '# a',
    },
  },
  {
    name: 'rust-tauri',
    tree: {
      'Cargo.toml': md([
        '[package]',
        'name = "app"',
        'rust-version = "1.85"',
        '',
        '[dependencies]',
        'tauri = "2.1"',
        'tauri-build = "2"',
      ]),
      'Cargo.lock': '',
      'rustfmt.toml': md([
        '# comment',
        'edition = "2024"',
        'max_width = 100',
        '[section]',
        'hard_tabs = true',
        'imports_granularity = "Crate"',
      ]),
      'package.json': json({ dependencies: { '@tauri-apps/api': '^2' } }),
    },
  },
  {
    name: 'cargo-mentions-tauri-only-in-a-comment',
    tree: {
      'Cargo.toml': md([
        '[package]',
        '# not a tauri project, tauri is only named here',
        'name = "x"',
      ]),
    },
  },
  {
    name: 'flutter',
    tree: {
      'pubspec.yaml': 'dependencies:\n  flutter:\n    sdk: flutter\n',
      'pubspec.lock': '',
      'README.md': 'App\n',
    },
  },
  { name: 'dart-without-flutter', tree: { 'pubspec.yaml': 'name: lib\n' } },
  {
    name: 'php-laravel',
    tree: {
      'composer.json': json({ require: { 'laravel/framework': '^11' } }),
      'README.md': '# Shop',
    },
  },
  { name: 'php-invalid-composer', tree: { 'composer.json': '{ not json' } },
  {
    name: 'php-require-is-a-string',
    tree: { 'composer.json': json({ require: 'laravel/framework' }) },
  },
  {
    name: 'python-and-java-merge-evidence',
    tree: {
      'pyproject.toml': '',
      'requirements.txt': '',
      'pom.xml': '',
      'build.gradle': '',
      'build.gradle.kts': '',
      'detekt.yml': 'x: 1',
    },
  },
  { name: 'go-ruby', tree: { 'go.mod': 'module x', Gemfile: '' } },
  {
    name: 'package-json-odd-shapes',
    tree: {
      'package.json': json({
        workspaces: [],
        packageManager: 'pnpm@9.1.0',
        engines: { node: 20 },
        dependencies: ['react'],
        devDependencies: { svelte: { weird: true }, electron: '' },
      }),
      'pnpm-lock.yaml': '',
      '.nvmrc': '20',
      'deno.jsonc': '{}',
    },
  },
  {
    name: 'package-manager-unknown',
    tree: { 'package.json': json({ packageManager: 'rush@5', engines: {} }) },
  },
  { name: 'package-json-null', tree: { 'package.json': 'null' } },
  { name: 'package-json-invalid', tree: { 'package.json': '{ nope' } },
  { name: 'package-json-array', tree: { 'package.json': '[]' } },
  {
    name: 'markdown-edge-cases',
    tree: {
      'README.md': '﻿# With BOM\r\n\r\nbody text\r\n',
      'CLAUDE.md':
        '# Line separator title\n## Heading\r\n### Another \r\nText\r## Lone carriage return\n##    spaced   \n',
      'CONTRIBUTING.md': '#   \n',
      'ARCHITECTURE.md': '',
      'ROADMAP.md': '   \n\t\n',
      'docs/a.md': '#\n\nTitle after blank',
      'docs/b.md': 'no title\n## sub\n',
      'docs/bom-only.md': '\uFEFF',
      'docs/nel.md': '\u0085 word \u0085',
      'docs/nbsp.md': '\u00a0one\u00a0two\u00a0',
    },
  },
  {
    name: 'configs-eslint-prettier',
    tree: {
      '.eslintrc.json': json({
        rules: {
          'no-var': 'error',
          semi: ['error', 'always'],
          '2': 'x',
          '1': { a: 1, b: [1, 2] },
          '10': 'ten',
          '9': 'nine',
        },
      }),
      '.prettierrc': json(['x', 'y']),
    },
  },
  { name: 'eslintrc-yaml-is-skipped', tree: { '.eslintrc': 'rules:\n  no-var: error\n' } },
  { name: 'eslintrc-rules-is-a-string', tree: { '.eslintrc.json': json({ rules: 'abc' }) } },
  {
    name: 'eslintrc-with-comments',
    tree: { '.eslintrc.json': '// comment\n{ "rules": { "a": 1 } }' },
  },
  {
    name: 'biome-shapes',
    tree: {
      'biome.json': json({
        linter: { rules: { recommended: false, a: ['x'], b: 'str', c: null, d: {} } },
        formatter: 'tabs',
      }),
    },
  },
  {
    name: 'biome-linter-false',
    tree: { 'biome.json': json({ linter: false, formatter: { enabled: false, lineWidth: 80 } }) },
  },
  {
    name: 'src-layers-only',
    tree: { 'src/domain/x.ts': '', 'src/application/y.ts': '', 'pnpm-workspace.yaml': '' },
  },
  { name: 'src-single-layer', tree: { 'src/domain/x.ts': '', 'src/modules/y.ts': '' } },
  {
    name: 'broken-symlink-in-docs',
    tree: { 'docs/broken.md': { symlink: 'does-not-exist' }, 'docs/ok.md': '# ok' },
  },
  {
    name: 'docs-symlink-to-dir-and-file',
    tree: { 'real/inner.md': '# inner', 'docs/link': { symlink: '../real' }, 'docs/x.md': '# x' },
  },
];

function materialize(root: string, tree: Tree) {
  for (const [path, entry] of Object.entries(tree)) {
    const full = join(root, path);
    mkdirSync(dirname(full), { recursive: true });
    if (typeof entry === 'string') writeFileSync(full, entry);
    else symlinkSync(entry.symlink, full);
  }
}

const out = cases.map(({ name, tree }) => {
  const root = mkdtempSync(join(tmpdir(), 'dg-intel-'));
  try {
    materialize(root, tree);
    try {
      const context = buildRepositoryContext(root);
      const text = JSON.stringify({ ...context, rootDir: '<root>', generatedAt: '<T>' });
      return {
        name,
        tree,
        expected: {
          context: JSON.parse(text),
          summary: summarizeCapabilities(context.capabilities),
        },
      };
    } catch (error) {
      return { name, tree, expected: { error: (error as Error).constructor.name } };
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

const target = 'crates/debuggatha-core/tests/fixtures/intelligence-conformance.json';
writeFileSync(target, JSON.stringify({ cases: out }));
const errors = out.filter((c) => 'error' in c.expected).length;
console.log(
  `wrote ${target}: ${out.length} repositories (${errors} that make the TypeScript scanners throw)`,
);
