/**
 * Error Mapping (epic) — "avoid leaking implementation details, preserve
 * enough information for debugging."
 *
 * Every domain package in this monorepo (repository-intelligence,
 * review-engine, knowledge-system, findings-ledger) throws plain `Error`
 * instances with deliberately-written, human-safe messages (no absolute
 * paths, no stack frames baked into the string) — see each package's own
 * source. Those are safe to relay to an MCP client as-is.
 *
 * Node's built-in filesystem errors are not: `ENOENT`/`EACCES`/etc.
 * messages routinely embed absolute paths (e.g. `findings-ledger`'s
 * `loadLedger` can surface a raw fs error). Those are detected by the
 * presence of an errno-style `.code` property and replaced with a
 * generic client-facing message — the full detail still goes to the
 * structured log, server-side only.
 */

export interface ClassifiedError {
  clientMessage: string;
  logDetail: Record<string, unknown>;
}

interface ErrnoLike {
  code?: unknown;
}

export function classifyError(error: unknown): ClassifiedError {
  if (error instanceof Error) {
    const code = (error as Error & ErrnoLike).code;
    if (typeof code === "string") {
      return {
        clientMessage: "A filesystem error occurred while accessing the repository.",
        logDetail: { message: error.message, code, stack: error.stack },
      };
    }
    return {
      clientMessage: error.message,
      logDetail: { message: error.message, stack: error.stack },
    };
  }
  return {
    clientMessage: "An unexpected error occurred.",
    logDetail: { value: String(error) },
  };
}

export interface ToolErrorResult {
  content: [{ type: "text"; text: string }];
  isError: true;
}

export function toToolErrorResult(error: unknown): ToolErrorResult {
  const { clientMessage } = classifyError(error);
  return { content: [{ type: "text", text: clientMessage }], isError: true };
}
