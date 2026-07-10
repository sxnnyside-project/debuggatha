import { describe, expect, it } from "vitest";
import { createLogger } from "./logging.js";

describe("createLogger", () => {
  it("writes structured JSON lines with timestamp/level/event/data", () => {
    const lines: string[] = [];
    const logger = createLogger({ minLevel: "debug", write: (line) => lines.push(line) });

    logger.info("tool.execution.start", { tool: "list_findings" });

    expect(lines).toHaveLength(1);
    const parsed = JSON.parse(lines[0] ?? "{}");
    expect(parsed).toMatchObject({
      level: "info",
      event: "tool.execution.start",
      data: { tool: "list_findings" },
    });
    expect(() => new Date(parsed.timestamp).toISOString()).not.toThrow();
  });

  it("filters out messages below the configured minimum level", () => {
    const lines: string[] = [];
    const logger = createLogger({ minLevel: "warn", write: (line) => lines.push(line) });

    logger.debug("noisy");
    logger.info("still noisy");
    logger.warn("this one shows");
    logger.error("and this one");

    expect(lines).toHaveLength(2);
  });

  it("defaults data to an empty object when omitted", () => {
    const lines: string[] = [];
    const logger = createLogger({ minLevel: "debug", write: (line) => lines.push(line) });

    logger.error("transport.error");

    expect(JSON.parse(lines[0] ?? "{}").data).toEqual({});
  });
});
