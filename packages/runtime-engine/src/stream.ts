import type { SemanticReviewResult, SemanticStreamEvent } from "./types.js";

/**
 * Drains a streaming execution to its final `SemanticReviewResult` — for
 * callers (like a Review Skill) that want a single result rather than
 * handling events themselves. Concatenates every `"token"` event's text
 * as a fallback: if the provider never emits a `"result"` event (some
 * providers only stream raw tokens, leaving parsing to the caller), the
 * accumulated text is parsed via `parseSemanticResponse` at the end
 * instead of being lost.
 */
export async function consumeToCompletion(
  stream: AsyncIterable<SemanticStreamEvent>,
  parse: (text: string) => SemanticReviewResult,
): Promise<{ result: SemanticReviewResult; rawText: string; error: string | undefined }> {
  let rawText = "";
  let result: SemanticReviewResult | undefined;
  let error: string | undefined;

  for await (const event of stream) {
    if (event.kind === "token") rawText += event.text;
    else if (event.kind === "result") result = event.result;
    else if (event.kind === "error") error = event.message;
  }

  if (result) return { result, rawText, error };
  return { result: parse(rawText), rawText, error };
}
