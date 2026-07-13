# Debuggatha

**The engineering reviewer that remembers** — every finding traced to a
real rule, tracked from open to resolved, judged against the repo you
actually wrote.

Debuggatha reads your repository before it opens its mouth — stack,
dependencies, linter/formatter config, ADRs, README intent — and reviews
your diff, a file, or the whole workspace against what it finds. Every
finding cites why it exists: a rule in a Review Pack, a convention your
own repo declares, a recognized standard (OWASP, WCAG), or evidence in
the code itself. No source, no finding.

## Why this isn't another AI chat panel

Copilot, Cursor, and Claude Code are built to **generate** code — fast,
capable, and that's their job. Debuggatha has exactly one job:
**review**, after something else (you, or one of those tools) has
written the code. It doesn't answer from general training-data
intuition — it builds a `RepositoryContext` from your actual stack and
rules first, then reviews against that. And it remembers: findings
persist across sessions in a versioned ledger with a real lifecycle
(open → acknowledged → resolved → dismissed → reopened), so a review
isn't a wall of text you re-read from scratch tomorrow.

## What you get in VS Code

- **Findings Ledger** view — every open, resolved, and dismissed
  finding, filterable, with one-click resolve/dismiss/reopen.
- **Repository Intelligence** view — see what Debuggatha detected about
  your stack before you ask for a review.
- **Review commands** — `Debuggatha: Review Entire Workspace`,
  `Review Current File`, `Review Selection`, `Review Pending Changes`
  (diff-scoped), all from the Command Palette or the editor toolbar.
- **Configurable review depth and packs** — `quick`, `full`, or
  `architectural`, with default Review Packs tuned per project.

## Getting started

1. Install the extension.
2. Open a workspace with a repository (git-initialized).
3. Run **Debuggatha: Review Entire Workspace** once to seed the
   Findings Ledger, or **Review Pending Changes** to review just your
   current diff.
4. Findings show up in the **Debuggatha** activity bar view — click one
   to jump to its location.

Debuggatha runs its default review through your MCP host's own
configured model (no keys to manage) or a local runtime (Ollama, LM
Studio) if you set one — see `debuggatha.runtime` in Settings.

## Also available as

- **MCP server** (`@debuggatha/mcp`) — for Claude Code, Claude Desktop,
  or any MCP-speaking host.
- **CLI** (`@debuggatha/cli`) — for terminal use and CI, no editor
  required.

See the [project README](https://github.com/sxnnyside-project/debuggatha#readme)
for the full picture, or [CONTRIBUTING.md](https://github.com/sxnnyside-project/debuggatha/blob/main/CONTRIBUTING.md)
to work on Debuggatha itself.

## License

MIT — see [LICENSE](https://github.com/sxnnyside-project/debuggatha/blob/main/LICENSE).
