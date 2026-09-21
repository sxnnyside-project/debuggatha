# @sxnnyside/debuggatha-cli

The standalone `debuggatha` command: terminal and CI usage, no editor required. It runs the same
reviews as the MCP server and the VS Code extension and reads and writes the same ledger.

## Commands

| Command | Purpose |
| --- | --- |
| `review` | Review a diff, files, or the repository |
| `baseline create\|show\|clear` | Record what the repository has today so later reviews report only what is new |
| `findings list\|show\|resolve\|dismiss\|reopen\|summary` | Work with the findings ledger |
| `memory list\|suggest\|confirm\|activate\|reject\|deprecate\|archive\|remove` | Repository Memory: accepted deviations, suppressions, exceptions, conventions |
| `repository` | What Debuggatha understood about the repository |
| `packs`, `policies` | Review Packs and the Review Policy a review would use |
| `analyzers` | Which external analyzers are installed, their licenses, and what a review would run |
| `trends` | How findings are moving, from the ledger's history |
| `init` | Write a config and a CI workflow (`--hooks` adds a pre-commit hook, `--force` overwrites files `init` wrote) |
| `doctor` | Diagnose the setup |
| `completion <bash\|zsh\|fish>` | Print a shell completion script |

Exit codes: `0` clean, `2` findings at or above `--fail-on`, `1` error. Machine formats own
stdout; progress and errors go to stderr. Color follows `NO_COLOR`, `FORCE_COLOR`, and whether the
output is a terminal.

## Reviewing

```bash
debuggatha review                                   # working tree changes, else the whole repository
debuggatha review --all                             # the whole repository, even with uncommitted changes
debuggatha review --files src/api.ts --no-persist
git diff | debuggatha review --diff - --format github
debuggatha review --changed-since origin/main --fail-on-new --fail-on high
debuggatha review --base origin/main...HEAD --format sarif --output debuggatha.sarif
```

| Option | Purpose |
| --- | --- |
| `--files`, `--diff` (`-` reads stdin), `--base <ref>`, `--all` | Scope: whole files, a diff, the changes since a ref, or everything |
| `--changed-since <ref>` | Review the whole files that changed since a ref, new and deleted ones included |
| `--fail-on <severity>` | Lowest severity that exits `2` (`informational` by default, `none` never fails) |
| `--fail-on-new` | Fail only on findings on lines the change added; the report says how many did not count |
| `--format text\|json\|sarif\|github\|markdown\|gitlab` | `sarif` feeds code scanning, `github` prints inline annotations, `markdown` is a pull-request comment, `gitlab` is the Code Quality report |
| `--output <file>` | Write the report to a file |
| `--no-persist` | Read-only: do not record findings in `.debuggatha/ledger.json` |
| `--include-baselined` | Show findings the baseline hides |
| `--pack <id...>` | Extra Review Packs on top of the ones detected for the stack |
| `--analyzer <id...>`, `--no-analyzers`, `--analyzer-timeout <s>` | Choose the external analyzers; those that run project code or use the network run only when named here |
| `--semantic ollama\|lmstudio`, `--semantic-model`, `--semantic-url`, `--semantic-verify` | Optional model pass; code goes only to `localhost` |

## Configuration

`.debuggatha/config.json` sets defaults a flag overrides. The nearest config at or above the working
directory applies, so a package in a monorepo finds the repository's.

```json
{
  "review": { "failOn": "high", "format": "text", "packs": [] },
  "analyzers": { "mode": "auto", "disable": [], "timeoutSeconds": 60 },
  "ignore": ["**/generated/**"],
  "overrides": [{ "paths": ["packages/legacy/**"], "failOn": "critical" }]
}
```

`ignore` leaves paths out of the report and the exit code; `overrides` set a different `failOn`
for some paths (first match wins). Patterns are globs relative to the directory that holds
`.debuggatha/`. A typo or a wrong value is an error that names it. A config can make a review
stricter or quieter; it cannot enable an analyzer that runs project code or uses the network, or
the semantic pass.

## Adopting on an existing repository

```bash
debuggatha baseline create        # today's findings
debuggatha review --fail-on high  # from now on, only what is new
debuggatha init                   # config + CI workflow (GitHub Actions or GitLab, from the remote or --ci)
```

## Install

```bash
bunx @sxnnyside/debuggatha-cli review          # no install
bun add --global @sxnnyside/debuggatha-cli
```

The bundle also runs on Node 20+: `npx @sxnnyside/debuggatha-cli review`. Standalone executables for macOS,
Linux, and Windows (arm64/x64) are attached to each `cli-v*` release; build them with
`just binaries`.

## Boundaries

The CLI is a thin adapter: argument parsing (`commander`), formatting, and exit codes. Every command
delegates to `@debuggatha/engine`; review logic belongs upstream of a command handler.
