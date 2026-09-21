# @debuggatha/mcp

The Debuggatha MCP server, for hosts that speak MCP (Claude Code, Claude Desktop, Cursor, Copilot
agent mode). It exposes the review engine as tools, resources, and prompts; it adds no review logic
of its own.

## Tools

| Tool | Reads or writes | Purpose |
| --- | --- | --- |
| `review_changes` | writes the ledger unless `persist: false` | Review what changed since the last commit or a ref (new and deleted files included); answers with `changes.introduced`, `changes.fixed`, and `changes.reopened` |
| `review_diff`, `review_files`, `review_workspace` | writes the ledger unless `persist: false` | Review a diff, files, or the repository against the packs that match its stack |
| `list_findings`, `get_finding` | read-only | Query the findings ledger |
| `update_finding` | writes the ledger | Move a finding through its lifecycle (history is append-only) |
| `explain_finding` | read-only | The reasoning behind a finding, the fix, and how to accept it |
| `suppress_finding` | writes memory | Accept a rule in a file durably, with a required reason |
| `create_baseline` | writes the baseline | Record today's findings so later reviews report only what is new |
| `repository_context`, `repository_summary` | read-only | What Debuggatha understood about the repository |
| `list_analyzers` | read-only | Installed external analyzers, their licenses, and what a review would run |

Every tool declares a title, `annotations`, and an `outputSchema`, and returns `structuredContent`
alongside the JSON text. Review tools return findings with a stable `ledgerId`, plus `baselined`,
`suppressedInline`, and an `analyzers` status; they accept `includeBaselined`, `analyzers: false`,
and `semantic: true`. Any tool takes `rootDir`; the repository defaults to
`DEBUGGATHA_REPOSITORY_ROOT`, then the first workspace root the client shares.

Resources: `debuggatha://repository/context`, `…/summary`, `…/ledger`, `…/ledger/history` (each also
with `?rootDir=`). Prompts: `review-flow` (the write, review, fix, verify loop), `fix-findings`, and
`triage-findings`. The server's instructions teach the loop to the connected agent.

## Transports

stdio by default. `debuggatha-mcp --http [--host 127.0.0.1] [--port 3333]` serves stateless
Streamable HTTP at `/mcp`. It binds to loopback, rejects foreign `Host` headers, and has **no
authentication**, so keep it on localhost or behind an authenticating proxy.

## Install

Claude Code:

```bash
claude mcp add debuggatha -- bunx @debuggatha/mcp
```

Any host that reads an `mcpServers` config (Claude Desktop, Cursor):

```json
{
  "mcpServers": {
    "debuggatha": {
      "command": "bunx",
      "args": ["@debuggatha/mcp"],
      "env": { "DEBUGGATHA_REPOSITORY_ROOT": "/path/to/your/repo" }
    }
  }
}
```

Without Bun, use Node: `npx -y @debuggatha/mcp`. The bundle runs the same on both; `just smoke`
checks each. `server.json` is the MCP Registry manifest; its version and `mcpName` must match
`package.json` (`just smoke` enforces it).

## Configuration

Set by whoever starts the server, never by a tool call.

| Variable | Default | Purpose |
| --- | --- | --- |
| `DEBUGGATHA_REPOSITORY_ROOT` | none | Repository reviewed when a tool call omits `rootDir` |
| `DEBUGGATHA_LOG_LEVEL` | `info` | `debug`, `info`, `warn`, or `error`; logs go to stderr |
| `DEBUGGATHA_CACHE_ENABLED` | `true` | Cache the resolved repository context between calls |
| `DEBUGGATHA_ANALYZERS` | `auto` | `off`, `auto` (tools that only read source), or a list of analyzer ids; the only way to enable one that runs project code or uses the network |
| `DEBUGGATHA_ANALYZER_TIMEOUT` | `60` | Seconds an analyzer may run |
| `DEBUGGATHA_SEMANTIC` | `off` | `sampling` (the connected client's own model), `ollama`, or `lmstudio` |
| `DEBUGGATHA_SEMANTIC_MODEL`, `_URL`, `_VERIFY` | none | Tune the local runtime; `_VERIFY=true` turns on verification |

A tool call can ask for the semantic pass but cannot choose where code goes, and an agent can turn
analyzers off for a call but cannot enable one.

## Boundaries

Zero business logic: every handler delegates to `@debuggatha/engine` through an injected
`DomainDeps`, and errors are sanitized so raw paths and stack traces never reach the client. The
package ships as one self-contained bundle that depends only on the MCP SDK and `zod`.
