/** Reads a fetch `Response` body as a sequence of decoded text chunks — shared by both NDJSON (Ollama) and SSE (LM Studio) parsing below. */
export async function* readTextChunks(response: Response): AsyncGenerator<string> {
  const body = response.body;
  if (!body) return;
  const reader = body.getReader();
  const decoder = new TextDecoder();
  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      yield decoder.decode(value, { stream: true });
    }
  } finally {
    reader.releaseLock();
  }
}

/** Ollama's `/api/generate` stream: one JSON object per newline-delimited line. */
export async function* readNdjsonLines(response: Response): AsyncGenerator<unknown> {
  let buffer = "";
  for await (const chunk of readTextChunks(response)) {
    buffer += chunk;
    let newlineIndex = buffer.indexOf("\n");
    while (newlineIndex !== -1) {
      const line = buffer.slice(0, newlineIndex).trim();
      buffer = buffer.slice(newlineIndex + 1);
      if (line) {
        try {
          yield JSON.parse(line);
        } catch {
          // A partial/malformed line — skip it rather than aborting the whole stream.
        }
      }
      newlineIndex = buffer.indexOf("\n");
    }
  }
  const rest = buffer.trim();
  if (rest) {
    try {
      yield JSON.parse(rest);
    } catch {
      // trailing partial line — ignored
    }
  }
}

/** OpenAI-compatible SSE stream (LM Studio, and any future OpenAI-compatible provider): lines of `data: {...}`, terminated by `data: [DONE]`. */
export async function* readServerSentEvents(response: Response): AsyncGenerator<unknown> {
  let buffer = "";
  for await (const chunk of readTextChunks(response)) {
    buffer += chunk;
    let newlineIndex = buffer.indexOf("\n");
    while (newlineIndex !== -1) {
      const line = buffer.slice(0, newlineIndex).trim();
      buffer = buffer.slice(newlineIndex + 1);
      newlineIndex = buffer.indexOf("\n");
      if (!line.startsWith("data:")) continue;
      const payload = line.slice("data:".length).trim();
      if (payload === "[DONE]") return;
      try {
        yield JSON.parse(payload);
      } catch {
        // malformed SSE payload — skip
      }
    }
  }
}
