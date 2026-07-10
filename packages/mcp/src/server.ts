import type { LedgerEntryFilter, RepositoryContextCache } from "@debuggatha/core";
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import {
  CallToolRequestSchema,
  GetPromptRequestSchema,
  ListPromptsRequestSchema,
  ListResourcesRequestSchema,
  ListToolsRequestSchema,
  ReadResourceRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { z } from "zod";
import type { ServerConfig } from "./config.js";
import { resolveRepositoryRoot } from "./config.js";
import type { DomainDeps } from "./domain-deps.js";
import type { Logger } from "./logging.js";
import { runReview } from "./tools/run-review.js";
import { jsonResult, type ToolTextResult, withToolLogging } from "./tools/shared.js";

export function createServer(
  deps: DomainDeps,
  config: ServerConfig,
  logger: Logger,
  cache: RepositoryContextCache | undefined,
): Server {
  const server = new Server(
    {
      name: "debuggatha",
      version: "0.1.0",
    },
    {
      capabilities: {
        tools: {},
        resources: {},
        prompts: {},
      },
    },
  );

  const ctx = { deps, config, logger, cache };

  // 1. Tool handlers configuration
  // biome-ignore lint/suspicious/noExplicitAny: input args are raw json, validated dynamically at runtime
  const toolHandlers: Record<string, (args: any) => Promise<ToolTextResult>> = {
    review_diff: withToolLogging(ctx, "review_diff", async (rawArgs) => {
      const args = z
        .object({
          rootDir: z.string().optional(),
          diff: z.string(),
          base: z.string().optional(),
          depth: z.enum(["quick", "full", "architectural"]).optional().default("full"),
          packIds: z.array(z.string()).optional().default([]),
          policyId: z.string().optional(),
        })
        .parse(rawArgs);

      const resolvedRoot = resolveRepositoryRoot(ctx.config, args.rootDir);
      const output = runReview(ctx, {
        rootDir: resolvedRoot,
        scope: { kind: "diff", diff: args.diff, base: args.base },
        depth: args.depth,
        packIds: args.packIds,
        policyId: args.policyId,
      });

      return jsonResult(output);
    }),

    review_files: withToolLogging(ctx, "review_files", async (rawArgs) => {
      const args = z
        .object({
          rootDir: z.string().optional(),
          paths: z.array(z.string()),
          depth: z.enum(["quick", "full", "architectural"]).optional().default("full"),
          packIds: z.array(z.string()).optional().default([]),
          policyId: z.string().optional(),
        })
        .parse(rawArgs);

      const resolvedRoot = resolveRepositoryRoot(ctx.config, args.rootDir);
      const output = runReview(ctx, {
        rootDir: resolvedRoot,
        scope: { kind: "files", paths: args.paths },
        depth: args.depth,
        packIds: args.packIds,
        policyId: args.policyId,
      });

      return jsonResult(output);
    }),

    review_workspace: withToolLogging(ctx, "review_workspace", async (rawArgs) => {
      const args = z
        .object({
          rootDir: z.string().optional(),
          depth: z.enum(["quick", "full", "architectural"]).optional().default("full"),
          packIds: z.array(z.string()).optional().default([]),
          policyId: z.string().optional(),
        })
        .parse(rawArgs);

      const resolvedRoot = resolveRepositoryRoot(ctx.config, args.rootDir);
      const output = runReview(ctx, {
        rootDir: resolvedRoot,
        scope: { kind: "workspace" },
        depth: args.depth,
        packIds: args.packIds,
        policyId: args.policyId,
      });

      return jsonResult(output);
    }),

    list_findings: withToolLogging(ctx, "list_findings", async (rawArgs) => {
      const args = z
        .object({
          rootDir: z.string().optional(),
          status: z
            .array(z.enum(["open", "acknowledged", "resolved", "dismissed", "reopened"]))
            .optional(),
          severity: z
            .array(z.enum(["informational", "low", "medium", "high", "critical"]))
            .optional(),
          category: z.array(z.string()).optional(),
          file: z.string().optional(),
        })
        .parse(rawArgs);

      const resolvedRoot = resolveRepositoryRoot(ctx.config, args.rootDir);
      const ledger = ctx.deps.loadLedger(resolvedRoot);
      const filter: LedgerEntryFilter = {};
      if (args.status !== undefined) filter.status = args.status;
      if (args.severity !== undefined) filter.severity = args.severity;
      if (args.category !== undefined) filter.category = args.category;
      if (args.file !== undefined) filter.file = args.file;
      const entries = ctx.deps.listEntries(ledger, filter);

      return jsonResult(entries);
    }),

    get_finding: withToolLogging(ctx, "get_finding", async (rawArgs) => {
      const args = z
        .object({
          rootDir: z.string().optional(),
          findingId: z.string(),
        })
        .parse(rawArgs);

      const resolvedRoot = resolveRepositoryRoot(ctx.config, args.rootDir);
      const ledger = ctx.deps.loadLedger(resolvedRoot);
      const entry = ctx.deps.getEntry(ledger, args.findingId);
      if (!entry) {
        throw new Error(`No ledger entry with id "${args.findingId}".`);
      }

      return jsonResult(entry);
    }),

    update_finding: withToolLogging(ctx, "update_finding", async (rawArgs) => {
      const args = z
        .object({
          rootDir: z.string().optional(),
          findingId: z.string(),
          status: z.enum(["open", "acknowledged", "resolved", "dismissed", "reopened"]),
          comment: z.string().optional(),
        })
        .parse(rawArgs);

      const resolvedRoot = resolveRepositoryRoot(ctx.config, args.rootDir);
      const ledger = ctx.deps.loadLedger(resolvedRoot);
      const updatedLedger = ctx.deps.updateFindingStatus(
        ledger,
        args.findingId,
        args.status,
        { kind: "manual", actor: "mcp-server" },
        args.comment,
      );
      ctx.deps.saveLedger(updatedLedger);
      const updatedEntry = ctx.deps.getEntry(updatedLedger, args.findingId);

      return jsonResult(updatedEntry);
    }),

    repository_context: withToolLogging(ctx, "repository_context", async (rawArgs) => {
      const args = z
        .object({
          rootDir: z.string().optional(),
        })
        .parse(rawArgs);

      const resolvedRoot = resolveRepositoryRoot(ctx.config, args.rootDir);
      const repoContext = ctx.deps.buildRepositoryContext(
        resolvedRoot,
        ctx.cache ? { cache: ctx.cache } : {},
      );

      return jsonResult(repoContext);
    }),

    repository_summary: withToolLogging(ctx, "repository_summary", async (rawArgs) => {
      const args = z
        .object({
          rootDir: z.string().optional(),
        })
        .parse(rawArgs);

      const resolvedRoot = resolveRepositoryRoot(ctx.config, args.rootDir);
      const repoContext = ctx.deps.buildRepositoryContext(
        resolvedRoot,
        ctx.cache ? { cache: ctx.cache } : {},
      );
      const ledger = ctx.deps.loadLedger(resolvedRoot);
      const ledgerSummary = ctx.deps.summarizeLedger(ledger);

      return jsonResult({
        rootDir: resolvedRoot,
        stack: repoContext.stack,
        dependencies: repoContext.dependencies,
        criteria: repoContext.criteria,
        ledgerSummary,
      });
    }),
  };

  // 2. Register List Tools Request
  server.setRequestHandler(ListToolsRequestSchema, async () => {
    return {
      tools: [
        {
          name: "review_diff",
          description: "Runs a code review on a git diff of the repository.",
          inputSchema: {
            type: "object",
            properties: {
              rootDir: {
                type: "string",
                description: "The repository root directory. Falls back to default config.",
              },
              diff: { type: "string", description: "The raw unified git diff text." },
              base: { type: "string", description: "The base commit/branch of the diff." },
              depth: {
                type: "string",
                enum: ["quick", "full", "architectural"],
                description: "Depth of the review.",
              },
              packIds: {
                type: "array",
                items: { type: "string" },
                description: "IDs of review packs to apply.",
              },
              policyId: { type: "string", description: "Optional specific policy ID to use." },
            },
            required: ["diff"],
          },
        },
        {
          name: "review_files",
          description: "Runs a code review on a list of specific files.",
          inputSchema: {
            type: "object",
            properties: {
              rootDir: {
                type: "string",
                description: "The repository root directory. Falls back to default config.",
              },
              paths: {
                type: "array",
                items: { type: "string" },
                description: "Array of file paths to review.",
              },
              depth: {
                type: "string",
                enum: ["quick", "full", "architectural"],
                description: "Depth of the review.",
              },
              packIds: {
                type: "array",
                items: { type: "string" },
                description: "IDs of review packs to apply.",
              },
              policyId: { type: "string", description: "Optional specific policy ID to use." },
            },
            required: ["paths"],
          },
        },
        {
          name: "review_workspace",
          description: "Runs a code review on the entire workspace repository.",
          inputSchema: {
            type: "object",
            properties: {
              rootDir: {
                type: "string",
                description: "The repository root directory. Falls back to default config.",
              },
              depth: {
                type: "string",
                enum: ["quick", "full", "architectural"],
                description: "Depth of the review.",
              },
              packIds: {
                type: "array",
                items: { type: "string" },
                description: "IDs of review packs to apply.",
              },
              policyId: { type: "string", description: "Optional specific policy ID to use." },
            },
          },
        },
        {
          name: "list_findings",
          description: "Lists findings stored in the repository findings ledger.",
          inputSchema: {
            type: "object",
            properties: {
              rootDir: {
                type: "string",
                description: "The repository root directory. Falls back to default config.",
              },
              status: {
                type: "array",
                items: {
                  type: "string",
                  enum: ["open", "acknowledged", "resolved", "dismissed", "reopened"],
                },
                description: "Filter by status.",
              },
              severity: {
                type: "array",
                items: {
                  type: "string",
                  enum: ["informational", "low", "medium", "high", "critical"],
                },
                description: "Filter by severity.",
              },
              category: {
                type: "array",
                items: { type: "string" },
                description: "Filter by category.",
              },
              file: { type: "string", description: "Filter by specific file path." },
            },
          },
        },
        {
          name: "get_finding",
          description: "Retrieves a single finding and its transition history.",
          inputSchema: {
            type: "object",
            properties: {
              rootDir: {
                type: "string",
                description: "The repository root directory. Falls back to default config.",
              },
              findingId: { type: "string", description: "Unique ID of the ledger entry." },
            },
            required: ["findingId"],
          },
        },
        {
          name: "update_finding",
          description: "Updates the lifecycle status of a finding in the ledger.",
          inputSchema: {
            type: "object",
            properties: {
              rootDir: {
                type: "string",
                description: "The repository root directory. Falls back to default config.",
              },
              findingId: { type: "string", description: "Unique ID of the ledger entry." },
              status: {
                type: "string",
                enum: ["open", "acknowledged", "resolved", "dismissed", "reopened"],
                description: "Target lifecycle status.",
              },
              comment: { type: "string", description: "Optional rationale comment." },
            },
            required: ["findingId", "status"],
          },
        },
        {
          name: "repository_context",
          description: "Returns the complete resolved context of the repository.",
          inputSchema: {
            type: "object",
            properties: {
              rootDir: {
                type: "string",
                description: "The repository root directory. Falls back to default config.",
              },
            },
          },
        },
        {
          name: "repository_summary",
          description:
            "Returns a high-level summary of stack, dependencies, criteria, and active findings.",
          inputSchema: {
            type: "object",
            properties: {
              rootDir: {
                type: "string",
                description: "The repository root directory. Falls back to default config.",
              },
            },
          },
        },
      ],
    };
  });

  // 3. Register Call Tool Request
  server.setRequestHandler(CallToolRequestSchema, async (request) => {
    const { name, arguments: args } = request.params;
    const handler = toolHandlers[name];
    if (!handler) {
      throw new Error(`Tool not found: ${name}`);
    }
    // biome-ignore lint/suspicious/noExplicitAny: handler returns ToolTextResult which satisfies CallToolResult
    return (await handler(args)) as any;
  });

  // 4. Register Resources
  server.setRequestHandler(ListResourcesRequestSchema, async () => {
    return {
      resources: [
        {
          uri: "debuggatha://repository/context",
          name: "Repository Context Snapshot",
          mimeType: "application/json",
          description:
            "The complete resolved repository context (stack, dependencies, criteria, documentation, understanding).",
        },
        {
          uri: "debuggatha://repository/summary",
          name: "Repository Summary",
          mimeType: "application/json",
          description:
            "High-level summary of the repository stack, dependencies, criteria, and ledger.",
        },
        {
          uri: "debuggatha://ledger",
          name: "Findings Ledger",
          mimeType: "application/json",
          description: "The complete findings ledger with all entries.",
        },
        {
          uri: "debuggatha://ledger/history",
          name: "Review & Findings Lifecycle History",
          mimeType: "application/json",
          description: "Chronological list of all ledger finding history events.",
        },
      ],
    };
  });

  server.setRequestHandler(ReadResourceRequestSchema, async (request) => {
    const uri = request.params.uri;
    let parsedUrl: URL;
    try {
      parsedUrl = new URL(uri);
    } catch (_err) {
      throw new Error(`Invalid resource URI: ${uri}`);
    }

    const rootDirParam = parsedUrl.searchParams.get("rootDir");
    const resolvedRoot = resolveRepositoryRoot(ctx.config, rootDirParam || undefined);

    let data: unknown;
    const path = parsedUrl.pathname || "";
    const resourceType = parsedUrl.host ? parsedUrl.host + path : path.replace(/^\/+/, "");

    if (resourceType === "repository/context") {
      data = ctx.deps.buildRepositoryContext(resolvedRoot, ctx.cache ? { cache: ctx.cache } : {});
    } else if (resourceType === "repository/summary") {
      const repoContext = ctx.deps.buildRepositoryContext(
        resolvedRoot,
        ctx.cache ? { cache: ctx.cache } : {},
      );
      const ledger = ctx.deps.loadLedger(resolvedRoot);
      const ledgerSummary = ctx.deps.summarizeLedger(ledger);
      data = {
        rootDir: resolvedRoot,
        stack: repoContext.stack,
        dependencies: repoContext.dependencies,
        criteria: repoContext.criteria,
        ledgerSummary,
      };
    } else if (resourceType === "ledger") {
      data = ctx.deps.loadLedger(resolvedRoot);
    } else if (resourceType === "ledger/history") {
      const ledger = ctx.deps.loadLedger(resolvedRoot);
      const events = ledger.entries.flatMap((entry) =>
        entry.history.map((h) => ({
          findingId: entry.id,
          fingerprint: entry.fingerprint,
          ...h,
        })),
      );
      events.sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());
      data = events;
    } else {
      throw new Error(`Resource not found: ${uri}`);
    }

    return {
      contents: [
        {
          uri,
          mimeType: "application/json",
          text: JSON.stringify(data, null, 2),
        },
      ],
    };
  });

  // 5. Register Prompts
  server.setRequestHandler(ListPromptsRequestSchema, async () => {
    return {
      prompts: [
        {
          name: "review-flow",
          description: "Assists in running a code review of the workspace or diff.",
          arguments: [
            {
              name: "rootDir",
              description: "The root directory of the repository",
              required: false,
            },
            {
              name: "depth",
              description: "The depth of the review (quick, full, architectural)",
              required: false,
            },
            {
              name: "packIds",
              description: "Comma-separated list of review pack IDs to run",
              required: false,
            },
          ],
        },
        {
          name: "triage-findings",
          description: "Assists in viewing and updating the status of active findings.",
          arguments: [
            {
              name: "rootDir",
              description: "The root directory of the repository",
              required: false,
            },
          ],
        },
      ],
    };
  });

  server.setRequestHandler(GetPromptRequestSchema, async (request) => {
    const name = request.params.name;
    const args = request.params.arguments || {};

    if (name === "review-flow") {
      const rootDir = args.rootDir || "";
      const depth = args.depth || "full";
      const packIds = args.packIds || "";
      return {
        messages: [
          {
            role: "user",
            content: {
              type: "text",
              text: `Please run a review on the workspace in "${rootDir}" with depth "${depth}"${packIds ? ` and packs [${packIds}]` : ""}. Use the \`review_workspace\` tool.`,
            },
          },
        ],
      };
    }

    if (name === "triage-findings") {
      const rootDir = args.rootDir || "";
      return {
        messages: [
          {
            role: "user",
            content: {
              type: "text",
              text: `Please list the current active findings in "${rootDir}" using the \`list_findings\` tool so we can triage them.`,
            },
          },
        ],
      };
    }

    throw new Error(`Unknown prompt: ${name}`);
  });

  return server;
}
