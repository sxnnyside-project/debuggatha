import type { RepositoryContextCache, SemanticProvider } from "@debuggatha/engine";
import type { ServerConfig } from "../config.js";
import type { DomainDeps } from "../domain-deps.js";
import { classifyError, toToolErrorResult } from "../errors.js";
import type { Logger } from "../logging.js";

/** Everything a tool handler needs, bundled once per server instance. */
export interface ToolRuntimeContext {
  deps: DomainDeps;
  config: ServerConfig;
  logger: Logger;
  cache: RepositoryContextCache | undefined;
  /** Resolves the repository a call targets: explicit argument, configured default, then the client's first root. */
  resolveRoot: (explicitRoot: string | undefined) => Promise<string>;
  /** The connected client's model, when the client supports sampling. */
  sampling: () => SemanticProvider | undefined;
}

export interface ToolResult {
  [key: string]: unknown;
  content: [{ type: "text"; text: string }];
  structuredContent?: Record<string, unknown>;
  isError?: boolean;
}

/** Structured payload plus its JSON text, as the spec asks for clients without structured-output support. */
export function structuredResult(payload: Record<string, unknown>): ToolResult {
  const plain = JSON.parse(JSON.stringify(payload)) as Record<string, unknown>;
  return {
    content: [{ type: "text", text: JSON.stringify(plain, null, 2) }],
    structuredContent: plain,
  };
}

/**
 * Wraps a tool's core logic with the cross-cutting concerns every tool
 * needs identically: start/success/failure logging with duration, and
 * converting a thrown domain error into a clean MCP error result instead
 * of letting it propagate raw. A handler only implements its translation
 * logic and lets errors throw naturally.
 */
export function withToolLogging<Args>(
  ctx: Pick<ToolRuntimeContext, "logger">,
  toolName: string,
  handler: (args: Args) => Promise<ToolResult> | ToolResult,
): (args: Args) => Promise<ToolResult> {
  return async (args: Args) => {
    const startedAt = Date.now();
    ctx.logger.info("tool.execution.start", { tool: toolName });
    try {
      const result = await handler(args);
      ctx.logger.info("tool.execution.success", {
        tool: toolName,
        durationMs: Date.now() - startedAt,
      });
      return result;
    } catch (error) {
      const { logDetail } = classifyError(error);
      ctx.logger.error("tool.execution.failure", {
        tool: toolName,
        durationMs: Date.now() - startedAt,
        ...logDetail,
      });
      return toToolErrorResult(error);
    }
  };
}
