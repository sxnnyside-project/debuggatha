import { type McpServer, ResourceTemplate } from "@modelcontextprotocol/sdk/server/mcp.js";
import { classifyError } from "./errors.js";
import { contextOptions } from "./tools/index.js";
import type { ToolRuntimeContext } from "./tools/shared.js";

interface ResourceSpec {
  name: string;
  title: string;
  description: string;
  path: string;
  read: (root: string, ctx: ToolRuntimeContext) => unknown;
}

const SPECS: ResourceSpec[] = [
  {
    name: "repository-context",
    title: "Repository context",
    description: "Stack, dependencies, documentation, criteria, and understanding.",
    path: "repository/context",
    read: (root, ctx) => ctx.deps.buildRepositoryContext(root, contextOptions(ctx)),
  },
  {
    name: "repository-summary",
    title: "Repository summary",
    description: "Stack, dependencies, criteria, and ledger totals.",
    path: "repository/summary",
    read: (root, ctx) => {
      const context = ctx.deps.buildRepositoryContext(root, contextOptions(ctx));
      return {
        rootDir: root,
        stack: context.stack,
        dependencies: context.dependencies,
        criteria: context.criteria,
        ledgerSummary: ctx.deps.summarizeLedger(ctx.deps.loadLedger(root)),
      };
    },
  },
  {
    name: "ledger",
    title: "Findings ledger",
    description: "Every finding the repository's ledger tracks.",
    path: "ledger",
    read: (root, ctx) => ctx.deps.loadLedger(root),
  },
  {
    name: "ledger-history",
    title: "Findings lifecycle history",
    description: "Every lifecycle event across all findings, newest first.",
    path: "ledger/history",
    read: (root, ctx) =>
      ctx.deps
        .loadLedger(root)
        .entries.flatMap((entry) =>
          entry.history.map((event) => ({
            findingId: entry.id,
            fingerprint: entry.fingerprint,
            ...event,
          })),
        )
        .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()),
  },
];

/**
 * `debuggatha://<path>` reads the default repository (configured root, or the client's
 * first root); `debuggatha://<path>?rootDir=/abs/path` reads another one.
 */
export function registerResources(server: McpServer, ctx: ToolRuntimeContext): void {
  for (const spec of SPECS) {
    const meta = { title: spec.title, description: spec.description, mimeType: "application/json" };
    const read = async (uri: URL, explicitRoot: string | undefined) => {
      try {
        const root = await ctx.resolveRoot(explicitRoot);
        return {
          contents: [
            {
              uri: uri.href,
              mimeType: "application/json",
              text: JSON.stringify(spec.read(root, ctx), null, 2),
            },
          ],
        };
      } catch (error) {
        const { clientMessage, logDetail } = classifyError(error);
        ctx.logger.error("resource.read.failure", { resource: spec.name, ...logDetail });
        throw new Error(clientMessage);
      }
    };

    server.registerResource(spec.name, `debuggatha://${spec.path}`, meta, (uri) =>
      read(uri, undefined),
    );
    server.registerResource(
      `${spec.name}-for-root`,
      new ResourceTemplate(`debuggatha://${spec.path}{?rootDir}`, { list: undefined }),
      meta,
      (uri, variables) =>
        read(
          uri,
          typeof variables.rootDir === "string" ? decodeURIComponent(variables.rootDir) : undefined,
        ),
    );
  }
}
