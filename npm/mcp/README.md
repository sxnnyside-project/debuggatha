# @sxnnyside/debuggatha-mcp

The Debuggatha MCP server, for hosts that speak MCP (Claude Code, Claude Desktop, Cursor, Copilot
agent mode). This package is a launcher: it carries no engine and starts the native `debuggatha mcp`,
which is the server. It finds the binary in `DEBUGGATHA_CLI`, then in the package of your platform
(`@sxnnyside/debuggatha-<os>-<cpu>`), then on `PATH`.

## Tools

Thirteen tools. Each declares a title, `annotations` (`readOnlyHint`, `destructiveHint`,
`idempotentHint`), an `outputSchema`, and returns `structuredContent` next to the JSON text.

| Tool | Reads or writes | Purpose |
| --- | --- | --- |
| `review_changes` | writes the ledger unless `persist: false` | Review what changed since the last commit or a ref (new and deleted files included); answers with `changes.introduced`, `changes.fixed`, and `changes.reopened` |
| `review_diff`, `review_files`, `review_workspace` | writes the ledger unless `persist: false` | Review a diff, files, or the repository against the packs that match its stack |
| `list_findings`, `get_finding` | read-only | Query the findings ledger; `list_findings` pages with `limit` and `cursor` |
| `update_finding` | writes the ledger | Move a finding through its lifecycle (history is append-only) |
| `explain_finding` | read-only | The reasoning behind a finding, the fix, and how to accept it |
| `suppress_finding` | writes memory (destructive) | Accept a rule in a file durably, with a required reason |
| `create_baseline` | overwrites the baseline (destructive) | Record today's findings so later reviews report only what is new |
| `repository_context`, `repository_summary` | read-only | What Debuggatha understood about the repository |
| `list_analyzers` | read-only | Installed external analyzers, their licenses, and what a review would run |

Review tools accept `includeBaselined`, `analyzers: false`, `limit` and `semantic: true`
(the last needs a client that can lend its model). Any tool takes `rootDir`, which must be the
repository the server started in, one given with `--allow-root`, or one the client shares as a root;
without it the repository is `DEBUGGATHA_REPOSITORY_ROOT`, then the first root the client shares.

Resources: `debuggatha://repository/context`, `debuggatha://repository/summary`, `debuggatha://ledger`, `debuggatha://ledger/history`. Prompts:
`review-flow`, `fix-findings`, `triage-findings`.

## Transports

stdio by default. `debuggatha-mcp --http [--host 127.0.0.1] [--port 3333]` serves stateless
Streamable HTTP at `/mcp`. It binds to loopback only and has **no authentication**, so keep it on
localhost. Every other flag of `debuggatha mcp` (`--allow-root`, analyzer and model choices) is
passed through.

## Install

Claude Code:

```bash
claude mcp add debuggatha -- bunx @sxnnyside/debuggatha-mcp
```

Any host that reads an `mcpServers` config (Claude Desktop, Cursor):

```json
{
  "mcpServers": {
    "debuggatha": {
      "command": "bunx",
      "args": ["@sxnnyside/debuggatha-mcp"],
      "env": { "DEBUGGATHA_REPOSITORY_ROOT": "/path/to/your/repo" }
    }
  }
}
```

Without Bun, use Node: `npx -y @sxnnyside/debuggatha-mcp`. The launcher runs the same on both. `server.json` is the MCP Registry manifest; its version and `mcpName` must match
`package.json` (`just smoke` enforces it).

## Configuration

Set by whoever starts the server, never by a tool call.

| Variable | Default | Purpose |
| --- | --- | --- |
| `DEBUGGATHA_REPOSITORY_ROOT` | none | Repository reviewed when a tool call omits `rootDir` |
| `DEBUGGATHA_ANALYZERS` | `auto` | `off`, `auto` (tools that only read source), or a list of analyzer ids; the only way to enable one that runs project code or uses the network |
| `DEBUGGATHA_SEMANTIC` | `off` | `host` (the connected client's own model; `sampling` is accepted too), `ollama`, or `lmstudio` |
| `DEBUGGATHA_SEMANTIC_MODEL`, `DEBUGGATHA_SEMANTIC_URL` | none | Model name and address of the local runtime; `localhost` only |

A tool call can ask for the semantic pass but cannot choose where code goes, and an agent can turn
analyzers off for a call but cannot enable one.

## Boundaries

This package is a launcher; the server is the `debuggatha mcp` binary. Every handler delegates to the engine, and errors are sanitized so raw paths and stack traces never reach the client.
