import { describe, expect, it } from "bun:test";
import { classifyError, toToolErrorResult } from "./errors.js";

describe("classifyError", () => {
  it("relays a plain domain Error's message as-is — these are already written to be safe", () => {
    const result = classifyError(new Error('No ledger entry with id "abc".'));
    expect(result.clientMessage).toBe('No ledger entry with id "abc".');
  });

  it("sanitizes an errno-style filesystem error instead of leaking its path", () => {
    const fsError = Object.assign(
      new Error(
        "ENOENT: no such file or directory, open '/Users/jane/secret/.debuggatha/ledger.json'",
      ),
      {
        code: "ENOENT",
      },
    );

    const result = classifyError(fsError);

    expect(result.clientMessage).not.toContain("/Users/jane");
    expect(result.clientMessage).toBe(
      "A filesystem error occurred while accessing the repository.",
    );
    expect(result.logDetail.code).toBe("ENOENT");
    expect(result.logDetail.message).toContain("/Users/jane");
  });

  it("still logs the full message and stack for a plain domain error, for server-side debugging", () => {
    const error = new Error("Illegal finding lifecycle transition");
    const result = classifyError(error);
    expect(result.logDetail.message).toBe("Illegal finding lifecycle transition");
    expect(result.logDetail.stack).toBeDefined();
  });

  it("handles a thrown non-Error value without crashing", () => {
    const result = classifyError("just a string");
    expect(result.clientMessage).toBe("An unexpected error occurred.");
  });
});

describe("toToolErrorResult", () => {
  it("shapes a classified error into an MCP tool error result", () => {
    const result = toToolErrorResult(new Error("boom"));
    expect(result).toEqual({ content: [{ type: "text", text: "boom" }], isError: true });
  });
});
