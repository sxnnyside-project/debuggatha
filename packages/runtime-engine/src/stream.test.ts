import { describe, expect, it } from "bun:test";
import { parseSemanticResponse } from "./parse-response.js";
import { consumeToCompletion } from "./stream.js";
import type { SemanticStreamEvent } from "./types.js";

async function* streamOf(events: SemanticStreamEvent[]): AsyncIterable<SemanticStreamEvent> {
  for (const event of events) yield event;
}

describe("consumeToCompletion", () => {
  it("prefers an explicit result event over accumulated tokens", async () => {
    const explicitResult = { candidates: [], parseWarnings: ["explicit"] };
    const { result, rawText } = await consumeToCompletion(
      streamOf([
        { kind: "token", text: "ignored" },
        { kind: "result", result: explicitResult },
      ]),
      parseSemanticResponse,
    );
    expect(result).toBe(explicitResult);
    expect(rawText).toBe("ignored");
  });

  it("falls back to parsing accumulated tokens when no result event arrives", async () => {
    const { result } = await consumeToCompletion(
      streamOf([
        { kind: "token", text: '{"candidates":[{"title":"t","explanation":"e",' },
        { kind: "token", text: '"confidence":"low","evidence":[{"file":"a.ts","excerpt":"x"}]}]}' },
        { kind: "done" },
      ]),
      parseSemanticResponse,
    );
    expect(result.candidates).toHaveLength(1);
  });

  it("surfaces an error event without throwing", async () => {
    const { error, result } = await consumeToCompletion(
      streamOf([{ kind: "error", message: "provider crashed" }]),
      parseSemanticResponse,
    );
    expect(error).toBe("provider crashed");
    expect(result.candidates).toEqual([]);
  });
});
