// Packs @sxnnyside/debuggatha-cli and @sxnnyside/debuggatha-mcp, installs the tarballs into an empty
// project, and runs the installed binaries under both Node and Bun (npx and bunx
// users get the same bundle) — proving each package is self-contained.
import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const RUNTIMES = ["node", "bun"];

const root = resolve(import.meta.dirname, "..");
const read = (path) => JSON.parse(readFileSync(join(root, path), "utf8"));
const run = (cmd, args, cwd) => execFileSync(cmd, args, { cwd, encoding: "utf8" }).trim();
const fail = (message) => {
  console.error(`smoke: ${message}`);
  process.exitCode = 1;
};

const cliPkg = read("packages/cli/package.json");
const mcpPkg = read("packages/mcp/package.json");
const server = read("packages/mcp/server.json");

if (server.version !== mcpPkg.version || server.packages[0].version !== mcpPkg.version) {
  fail(
    `server.json versions (${server.version}) must equal @sxnnyside/debuggatha-mcp ${mcpPkg.version}`,
  );
}
if (server.name !== mcpPkg.mcpName) fail("server.json name must equal package.json mcpName");

/** Speaks MCP over stdio to the installed server and lists its tools. */
async function handshake(runtime, script, cwd) {
  const child = spawn(runtime, [script], { cwd, stdio: ["pipe", "pipe", "ignore"] });
  const pending = new Map();
  let buffer = "";
  child.stdout.on("data", (chunk) => {
    buffer += chunk;
    for (let i = buffer.indexOf("\n"); i >= 0; i = buffer.indexOf("\n")) {
      const line = buffer.slice(0, i);
      buffer = buffer.slice(i + 1);
      try {
        const message = JSON.parse(line);
        pending.get(message.id)?.(message);
      } catch {
        // non-JSON output is ignored
      }
    }
  });
  let nextId = 0;
  const rpc = (method, params) =>
    new Promise((resolveRpc, reject) => {
      const id = ++nextId;
      const timer = setTimeout(() => reject(new Error(`${method} timed out`)), 15_000);
      pending.set(id, (message) => {
        clearTimeout(timer);
        resolveRpc(message);
      });
      child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
    });

  try {
    const init = await rpc("initialize", {
      protocolVersion: "2025-06-18",
      capabilities: {},
      clientInfo: { name: "smoke", version: "0" },
    });
    child.stdin.write(
      `${JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" })}\n`,
    );
    const tools = await rpc("tools/list", {});
    return {
      version: init.result?.serverInfo?.version,
      tools: tools.result?.tools?.map((tool) => tool.name) ?? [],
    };
  } finally {
    child.kill();
  }
}

const work = mkdtempSync(join(tmpdir(), "debuggatha-smoke-"));
try {
  const tarballs = join(work, "tarballs");
  const app = join(work, "app");
  mkdirSync(tarballs);
  mkdirSync(app);
  const packed = ["cli", "mcp"].map((name) =>
    join(
      tarballs,
      run(
        "npm",
        ["pack", "--silent", "--pack-destination", tarballs],
        join(root, "packages", name),
      ),
    ),
  );
  run("npm", ["init", "-y"], app);
  run("npm", ["install", "--silent", "--no-audit", "--no-fund", ...packed], app);

  const installed = (pkg) =>
    join(app, "node_modules", "@sxnnyside", `debuggatha-${pkg}`, "dist", "bin.js");

  for (const runtime of RUNTIMES) {
    const version = run(runtime, [installed("cli"), "--version"], app);
    if (version !== cliPkg.version) {
      fail(`[${runtime}] debuggatha --version printed "${version}", expected ${cliPkg.version}`);
    } else console.log(`ok  [${runtime}] debuggatha --version -> ${version}`);

    const mcp = await handshake(runtime, installed("mcp"), app);
    if (mcp.version !== mcpPkg.version) {
      fail(`[${runtime}] MCP serverInfo.version is "${mcp.version}", expected ${mcpPkg.version}`);
    } else if (mcp.tools.length === 0) {
      fail(`[${runtime}] MCP server exposes no tools`);
    } else console.log(`ok  [${runtime}] debuggatha-mcp handshake -> ${mcp.tools.length} tools`);
  }
} finally {
  rmSync(work, { recursive: true, force: true });
}
