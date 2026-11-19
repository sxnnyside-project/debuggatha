# @sxnnyside/debuggatha-cli

A senior code reviewer that cites a real source for every finding. This package is a launcher: it
carries no engine and runs the native `debuggatha` binary, which npm installs for your platform as
an optional dependency (`@sxnnyside/debuggatha-<os>-<cpu>`).

## Install

```bash
npm i -g @sxnnyside/debuggatha-cli     # or: bun add --global @sxnnyside/debuggatha-cli
```

## Usage

```bash
npx @sxnnyside/debuggatha-cli review
```

## Platforms

macOS arm64, Linux x64 and arm64, Windows x64. On any other, install the binary with
`cargo install debuggatha-cli` or Homebrew, and the launcher finds it on `PATH`; `DEBUGGATHA_CLI`
names a binary explicitly. If your installer skips optional dependencies (`--no-optional`), the
launcher says so.

## API

The MCP server is the same binary: [`@sxnnyside/debuggatha-mcp`](../mcp/README.md). Commands, exit
codes and options are in the [repository](https://github.com/sxnnyside-project/debuggatha#readme).
