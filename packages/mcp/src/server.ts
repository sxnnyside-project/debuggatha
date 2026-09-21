import { fileURLToPath } from "node:url";
import type { RepositoryContextCache } from "@debuggatha/engine";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import pkg from "../package.json" with { type: "json" };
import type { ServerConfig } from "./config.js";
import { resolveRepositoryRoot } from "./config.js";
import type { DomainDeps } from "./domain-deps.js";
import type { Logger } from "./logging.js";
import { registerPrompts } from "./prompts.js";
import { registerResources } from "./resources.js";
import { clientCanSample, samplingProvider } from "./sampling.js";
import { registerTools } from "./tools/index.js";
import type { ToolRuntimeContext } from "./tools/shared.js";

export function createServer(
  deps: DomainDeps,
  config: ServerConfig,
  logger: Logger,
  cache: RepositoryContextCache | undefined,
): McpServer {
  const server = new McpServer(
    { name: "debuggatha", title: "Debuggatha", version: pkg.version },
    {
      instructions:
        "Debuggatha reviews code against the Review Packs that match the repository's stack, and every finding cites the rule and evidence behind it. After you write or edit code, call review_changes, fix what changes.introduced lists (each recommendation says how, sometimes with an exact edit), and call it again until nothing new is introduced; changes.fixed confirms what your fixes resolved. Use explain_finding for the reasoning behind a finding. Accept a finding with suppress_finding only when the code is right and the rule is wrong, always with a real reason, and tell the user. Review tools record findings in the repository's ledger unless persist is false. When the server has a model configured, semantic: true also has a model read the changed code; what it raises is a labeled suspicion to check, never a verdict.",
    },
  );

  const ctx: ToolRuntimeContext = {
    deps,
    config,
    logger,
    cache,
    sampling: () => (clientCanSample(server) ? samplingProvider(server) : undefined),
    resolveRoot: async (explicitRoot) => {
      if (explicitRoot || config.defaultRepositoryRoot) {
        return resolveRepositoryRoot(config, explicitRoot);
      }
      if (server.server.getClientCapabilities()?.roots) {
        const { roots } = await server.server.listRoots();
        const first = roots.find((root) => root.uri.startsWith("file://"));
        if (first) return fileURLToPath(first.uri);
      }
      return resolveRepositoryRoot(config, undefined);
    },
  };

  registerTools(server, ctx);
  registerResources(server, ctx);
  registerPrompts(server);
  return server;
}
