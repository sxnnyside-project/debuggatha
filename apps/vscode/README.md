# Debuggatha

**The engineering reviewer that remembers**: every finding traced to a real rule, tracked from open
to resolved, judged against the repository you actually wrote.

Debuggatha reads your repository before it reviews anything (stack, dependencies, linter and
formatter config, ADRs, README intent) and reviews your diff, a file, a selection, or the whole
workspace against what it finds. Every finding cites why it exists: a rule in a Review Pack, a
convention your own repository declares, a recognized standard (OWASP, WCAG), the output of an
installed analyzer, or evidence in the code itself. No source, no finding.

## Not another AI chat panel

Copilot, Cursor, and Claude Code generate code. Debuggatha has one job: review, after something
else (you, or one of those tools) has written the code. It builds a picture of your stack and rules
first, then reviews against that. Findings persist across sessions in a versioned ledger with a real
lifecycle (open, acknowledged, resolved, dismissed, reopened), so a review is something you track,
not a wall of text you re-read.

## What you get

- **In the editor**: findings underline exactly the text they are about, with the rule as the
  diagnostic code. Hover shows the rule and its pack, why it matters, the fix with its before and
  after, and links to act on it.
- **Quick fixes**: the lightbulb applies the exact edit for findings that have one (`no-var`,
  `eqeqeq`, `no-explicit-any`, `avoid-print`), preferred only when the edit is safe, and offers
  nothing once the line no longer matches what the review read. **Accept Finding in Code** inserts a
  `debuggatha-ignore-next-line` comment with a required reason.
- **Findings view**: open findings grouped by file, with a toggle for resolved and dismissed ones,
  and resolve, dismiss, and reopen actions. A **Repository Intelligence** view shows what was
  detected about your stack. The status bar shows the open count.
- **Review commands**: `Review Entire Workspace`, `Review Current File`, `Review Selection`,
  `Review Pending Changes`, `Cancel Review`, and `Show Log`.
- **External analyzers**: gitleaks, Biome, Ruff, ktlint, detekt and others already on your machine
  run as separate processes and appear in the same view, labeled with tool, version, and license.
- **Reviews in their own process**: a slow analyzer never freezes the editor, and cancelling stops
  the review and the analyzers it started.

## Getting started

1. Install the extension and open a repository.
2. Run **Debuggatha: Review Entire Workspace** to seed the ledger, or **Review Pending Changes** for
   your current diff.
3. Findings appear in the **Debuggatha** activity bar view and the Problems panel; click one to jump
   to it.

## Settings

| Setting | Default | Purpose |
| --- | --- | --- |
| `debuggatha.reviewDepth` | `full` | `quick` runs the built-in detectors only; `full` also runs installed analyzers; `architectural` also reviews the whole repository, architecture included |
| `debuggatha.reviewOnSave` | `false` | Review the saved file quietly (status bar only), after a short pause; never widened by `reviewDepth` |
| `debuggatha.minimumSeverity` | `informational` | Lowest severity shown as underlines and in Problems; the Findings view lists everything |
| `debuggatha.extraReviewPacks` | `[]` | Review Packs applied on top of the ones detected for your stack |
| `debuggatha.logLevel` | `info` | What the Output channel records; it never contains code |
| `debuggatha.enabledAnalyzers` | `[]` | Analyzers that run project code (`eslint`, `phpstan`, `clippy`) or use the network (`osv-scanner`); machine scope |
| `debuggatha.semantic.provider`, `.model`, `.url` | `off` | A model on this machine for **Review Current File with a Local Model**; machine scope, `localhost` only |

## Trust

In a workspace you have not trusted, only the built-in detectors run: an analyzer found inside the
workspace (`node_modules/.bin/biome`) is code the workspace supplied. Analyzer and local-model
settings are machine scope, so a repository's `.vscode/settings.json` cannot switch them on. Code
goes to a model only when you run **Review Current File with a Local Model**, and only to a model on
this machine.

## Also available as

- **MCP server** (`@debuggatha/mcp`) for Claude Code, Claude Desktop, or any MCP host.
- **CLI** (`@debuggatha/cli`) for terminals and CI, no editor required.

See the [project README](https://github.com/sxnnyside-project/debuggatha#readme), or
[CONTRIBUTING.md](https://github.com/sxnnyside-project/debuggatha/blob/main/CONTRIBUTING.md) to work
on Debuggatha itself.

## License

MIT, see [LICENSE](https://github.com/sxnnyside-project/debuggatha/blob/main/LICENSE).
