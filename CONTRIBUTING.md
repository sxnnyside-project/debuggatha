# Contributing to Debuggatha

Contributions are welcome — bugs, fixes, features, or documentation.
This document covers how to work with the project as a contributor.

---

## Before You Start

- Search [existing issues](https://github.com/sxnnyside-project/debuggatha/issues) before opening a new one.
- For significant changes, open an issue first to discuss the direction before writing code.
- Read the [Code of Conduct](CODE_OF_CONDUCT.md). It applies to all interactions in this project.

---

## Reporting a Bug

Open a [GitHub Issue](https://github.com/sxnnyside-project/debuggatha/issues/new/choose) using the bug report template.

Include:
- What you expected to happen
- What actually happened
- Steps to reproduce
- Environment details (OS, runtime version, relevant config)

---

## Proposing a Feature

Open a [GitHub Issue](https://github.com/sxnnyside-project/debuggatha/issues/new/choose) using the feature request template, or submit a PR directly if the change is small and self-contained.

For larger features, an issue discussion first avoids wasted effort on both sides.

---

## Workflow

1. Fork the repository and create a branch from `main`.
2. Name your branch descriptively — `fix/crash-on-empty-input`, `feat/offline-mode`.
3. Set up the toolchains: `just install` (needs [Bun](https://bun.sh), Node with [pnpm](https://pnpm.io), and [just](https://github.com/casey/just)).
4. Make your changes, with tests. `just check` runs what CI runs: format check, lint, typecheck, tests, build, and the license gate.
5. When a change touches the CLI or MCP packaging, run `just smoke`; when it touches the extension, run `just test-vscode`.
6. Open a pull request against `main` with a clear description of what changed and why.

Git hooks (installed with the dependencies) format staged files on commit, run typecheck and tests on push, and check the commit message.

Read [CLAUDE.md](CLAUDE.md) first if you change how findings are produced: every finding must cite evidence, and adapters import only `@debuggatha/engine`.

---

## Pull Request Checklist

Before submitting:

- [ ] `just check` passes
- [ ] Changes are described in [CHANGELOG.md](CHANGELOG.md) under `[Unreleased]`
- [ ] The PR description explains what changed and why
- [ ] New behavior is covered by tests where applicable

---

## Commit Style

This project uses [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/). Every commit message must follow the format:

```
<type>: <description>

[optional body]
[optional footer]
```

Accepted types:

| Type       | Use for                                          |
|------------|--------------------------------------------------|
| `feat`     | New functionality                                |
| `fix`      | Bug fixes                                        |
| `docs`     | Documentation only                               |
| `style`    | Formatting, whitespace — no logic changes        |
| `refactor` | Code restructure without behavior change         |
| `test`     | Adding or updating tests                         |
| `chore`    | Build process, tooling, dependencies             |
| `perf`     | Performance improvements                         |

Examples:

```
feat: add offline fallback for config reads
fix: prevent crash when scripts directory is missing
docs: update installation steps for cross-compilation
chore: bump dependencies to latest stable
```

Commits that don't follow this format will be flagged during review.

---

## Questions

If something in the codebase is unclear, open an issue with the `question` label before assuming it's a bug.

---

*Debuggatha is A Sxnnyside Project. Part of the [Sxnnyside Project](https://sxnnysideproject.com).*
