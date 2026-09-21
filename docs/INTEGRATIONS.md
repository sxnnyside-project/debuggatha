# Using Debuggatha with your coding tool

Debuggatha runs as an MCP server (`debuggatha-mcp`), a CLI (`debuggatha`), and a VS Code extension.
Any coding agent that speaks MCP can call it after it writes code: the agent writes, Debuggatha
reviews, and the findings land in the same ledger the CLI and the editor read.

## 1. Pick how the host launches the server

| Install | Command the host runs |
| --- | --- |
| No install (published package) | `bunx @debuggatha/mcp` or `npx -y @debuggatha/mcp` |
| Installed from this checkout | `debuggatha-mcp` (after `just install-cli`) |
| Installed from a registry | `debuggatha-mcp` (after `bun add --global @debuggatha/mcp` or `npm install --global @debuggatha/mcp`) |

The examples below use `debuggatha-mcp`; substitute `bunx` with `["@debuggatha/mcp"]` or `npx` with
`["-y", "@debuggatha/mcp"]` if you did not install it. A host that starts from a GUI may not see your
shell `PATH`; use the absolute path (`which debuggatha-mcp`) if it cannot find the command.

Every host needs the same three things: a name (`debuggatha`), a command with its arguments, and
optionally environment variables (see [Configuration](#3-configuration)).

## 2. Register it in your host

Config formats belong to each host and change between versions; if one below no longer works, the
host's own MCP documentation is authoritative. The command and arguments stay the same.

### Claude Code

```bash
claude mcp add debuggatha -- debuggatha-mcp                      # this project only
claude mcp add --scope user debuggatha -- debuggatha-mcp        # every project
claude mcp add debuggatha -e DEBUGGATHA_ANALYZERS=auto -- debuggatha-mcp
```

Check with `claude mcp list`, or `/mcp` inside a session.

### Qwen Code and Gemini CLI

Both read an `mcpServers` block from `settings.json` (`~/.qwen/settings.json` or `.qwen/settings.json`
for Qwen Code; `~/.gemini/settings.json` or `.gemini/settings.json` for Gemini CLI):

```json
{
  "mcpServers": {
    "debuggatha": {
      "command": "debuggatha-mcp",
      "args": [],
      "env": { "DEBUGGATHA_LOG_LEVEL": "info" }
    }
  }
}
```

Use `/mcp` inside the tool to see the server and its tools.

### Cursor

`.cursor/mcp.json` in the project, or `~/.cursor/mcp.json` for every project:

```json
{ "mcpServers": { "debuggatha": { "command": "debuggatha-mcp" } } }
```

### VS Code (Copilot agent mode)

`.vscode/mcp.json` in the workspace:

```json
{ "servers": { "debuggatha": { "type": "stdio", "command": "debuggatha-mcp" } } }
```

### Claude Desktop

`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS
(`%APPDATA%\Claude\claude_desktop_config.json` on Windows):

```json
{ "mcpServers": { "debuggatha": { "command": "debuggatha-mcp" } } }
```

Restart the app after editing. Claude Desktop does not inherit your shell `PATH`, so use the absolute
path of the command.

### Codex CLI

`~/.codex/config.toml`:

```toml
[mcp_servers.debuggatha]
command = "debuggatha-mcp"
args = []
```

### Windsurf, Cline, Continue, Zed, OpenCode

- **Windsurf**: `~/.codeium/windsurf/mcp_config.json`, same `mcpServers` shape as Cursor.
- **Cline**: the MCP Servers panel, or `cline_mcp_settings.json`, same `mcpServers` shape.
- **Continue**: a `.continue/mcpServers/debuggatha.yaml` block with `command: debuggatha-mcp`.
- **Zed**: `settings.json`, `"context_servers": { "debuggatha": { "command": "debuggatha-mcp", "args": [] } }`.
- **OpenCode**: `opencode.json`, `"mcp": { "debuggatha": { "type": "local", "command": ["debuggatha-mcp"] } }`.

### Any other MCP host

Point it at the command as a stdio server. For hosts that connect over HTTP, run
`debuggatha-mcp --http --port 3333` and use `http://127.0.0.1:3333/mcp`. The HTTP transport has no
authentication: keep it on localhost.

## 3. Configuration

Set these in the host's `env` block (or your shell) before the server starts. A tool call cannot
change them.

| Variable | Purpose |
| --- | --- |
| `DEBUGGATHA_REPOSITORY_ROOT` | Repository to review when a tool call gives no `rootDir`. Hosts that share workspace roots need not set it. |
| `DEBUGGATHA_ANALYZERS` | `off`, `auto` (default: only tools that read source), or a list of analyzer ids. The only way to enable `eslint`, `phpstan`, `clippy` (they run project code) or `osv-scanner` (uses the network). |
| `DEBUGGATHA_SEMANTIC` | `off` (default), `sampling` (the host's own model), `ollama`, or `lmstudio`. |
| `DEBUGGATHA_SEMANTIC_MODEL`, `_URL` | Model name and address for `ollama` or `lmstudio`; `localhost` only. |
| `DEBUGGATHA_LOG_LEVEL` | `debug`, `info`, `warn`, `error`; logs go to stderr. |

The optional model pass never changes what the deterministic review found. `sampling` works only in
hosts that support MCP sampling; elsewhere use `ollama` or `lmstudio`, or leave it off.

## 4. Teach the agent to use it

MCP hosts show the tools to the model, but a short instruction makes the loop reliable. Put this in
the file your tool reads (`CLAUDE.md`, `QWEN.md`, `GEMINI.md`, `AGENTS.md`, `.cursor/rules`, ...):

```markdown
## Code review

After you change code, call the Debuggatha `review_changes` tool and fix what it reports as
`introduced`. Use `explain_finding` before disputing a finding. Accept a finding only with
`suppress_finding` and a real reason. Do not edit `.debuggatha/` by hand.
```

The server also ships the `review-flow` and `fix-findings` prompts and instructions that describe
this loop, so hosts that surface prompts can offer them directly.

## 5. The CLI

The same reviews from a terminal or CI:

```bash
debuggatha review                       # what changed since the last commit
debuggatha review --all                 # the whole repository
debuggatha findings list                # the ledger
debuggatha baseline create              # adopt on an existing repository
debuggatha init                         # config and a GitHub Actions or GitLab pipeline
debuggatha doctor                       # diagnose the setup
```

See [packages/cli/README.md](../packages/cli/README.md) for every command and flag.

## 6. Installing from a checkout

```bash
just install          # toolchains
just install-cli      # puts debuggatha and debuggatha-mcp on your PATH (needs ~/.bun/bin on PATH)
just uninstall-cli    # undoes install-cli
just dev              # rebuilds the packages and watches the extension
```

To try the extension from source, open this repository in your editor (VS Code, Antigravity,
Cursor, or any VS Code fork) and press **F5** (**Run Extension**). The editor you are in opens its
own Extension Development Host with Debuggatha loaded; `just dev` in a terminal keeps it rebuilding.
To install a build instead, run `just package` and use **Extensions: Install from VSIX...** in your
editor, or install from Open VSX.

## 7. Troubleshooting

- **The host says the server failed to start**: run `debuggatha-mcp` in a terminal; it should wait
  silently on stdin. If the command is not found, use the absolute path.
- **No tools appear**: restart the host, then check its MCP panel or `/mcp`. There should be 13 tools.
- **An analyzer is missing from results**: `debuggatha analyzers` shows what is installed and what a
  review would run.
- **Findings do not match the editor**: they share `.debuggatha/ledger.json` in the repository root;
  check that both point at the same repository.
