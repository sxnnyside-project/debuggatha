import type { RepositoryContextCache } from "@debuggatha/core";
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
}

export interface ToolTextResult {
  content: [{ type: "text"; text: string }];
  isError?: boolean;
}

export function jsonResult(payload: unknown): ToolTextResult {
  return { content: [{ type: "text", text: JSON.stringify(payload, null, 2) }] };
}

/**
 * Wraps a tool's core logic with the cross-cutting concerns every tool
 * needs identically: start/success/failure logging with duration (epic
 * "Logging"), and converting a thrown domain error into a clean MCP
 * error result instead of letting it propagate raw (epic "Error Mapping").
 * A tool's own handler function only needs to implement its translation
 * logic and let errors throw naturally.
 */
export function withToolLogging<Args>(
  ctx: Pick<ToolRuntimeContext, "logger">,
  toolName: string,
  handler: (args: Args) => Promise<ToolTextResult> | ToolTextResult,
): (args: Args) => Promise<ToolTextResult> {
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
