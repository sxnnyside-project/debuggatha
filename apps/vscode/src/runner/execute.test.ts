import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { loadLedger } from "@debuggatha/engine";
import { afterEach, describe, expect, it } from "vitest";
import { executeJob } from "./execute.js";

let server: Server | undefined;
let root: string | undefined;
afterEach(() => {
  server?.close();
  server = undefined;
  if (root) rmSync(root, { recursive: true, force: true });
  root = undefined;
});

/** A runtime that answers like Ollama: a model list, and a streamed answer per prompt. */
function fakeOllama(answer: string): Promise<{ url: string; prompts: string[] }> {
  const prompts: string[] = [];
  server = createServer((request, response) => {
    if (request.url === "/api/tags") {
      response.end(JSON.stringify({ models: [{ name: "fake:1b" }] }));
      return;
    }
    let body = "";
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      prompts.push(body);
      response.end(
        `${JSON.stringify({ response: answer, done: false })}\n${JSON.stringify({ done: true })}\n`,
      );
    });
  });
  return new Promise((resolve) => {
    const listening = server as Server;
    listening.listen(0, "127.0.0.1", () => {
      const { port } = listening.address() as { port: number };
      resolve({ url: `http://127.0.0.1:${port}`, prompts });
    });
  });
}

function project(source: string) {
  root = mkdtempSync(join(tmpdir(), "debuggatha-exec-"));
  writeFileSync(join(root, "package.json"), JSON.stringify({ name: "x" }));
  mkdirSync(dirname(join(root, "src", "a.ts")), { recursive: true });
  writeFileSync(join(root, "src", "a.ts"), source);
  return root;
}

const claim = JSON.stringify({
  issues: [
    {
      line: 2,
      quote: "i <= items.length",
      title: "Off by one",
      why: "Reads one past the end.",
      category: "reliability",
      severity: "medium",
    },
  ],
});
const CODE =
  "export function total(items: number[]) {\n  for (let i = 0; i <= items.length; i++) {}\n}\n";

describe("executeJob with a model on this machine", () => {
  it("adds what the model suspects to the ledger, labeled, and reports what it did", async () => {
    const dir = project(CODE);
    const { url } = await fakeOllama(claim);

    const result = await executeJob({
      rootDir: dir,
      scope: { kind: "files", paths: [join(dir, "src", "a.ts")] },
      packIds: [],
      analyzers: { mode: "off" },
      semantic: { provider: "ollama", url },
    });

    expect(result.semantic).toMatchObject({ provider: "ollama", model: "fake:1b", detected: 1 });
    const raised = loadLedger(dir).entries.find((e) =>
      e.fingerprint.ruleId?.startsWith("semantic:"),
    );
    expect(raised?.latestFinding.confidence).toBe("low");
  });

  it("without a model, nothing is asked of one", async () => {
    const dir = project(CODE);
    const result = await executeJob({
      rootDir: dir,
      scope: { kind: "workspace" },
      packIds: [],
      analyzers: { mode: "off" },
    });
    expect(result.semantic).toBeNull();
  });

  it("refuses a runtime that is not on this machine before sending any code", async () => {
    const dir = project(CODE);
    await expect(
      executeJob({
        rootDir: dir,
        scope: { kind: "workspace" },
        packIds: [],
        analyzers: { mode: "off" },
        semantic: { provider: "ollama", url: "https://example.com" },
      }),
    ).rejects.toThrow("only to a model on this machine");
  });

  it("reports a runtime that is not running", async () => {
    const dir = project(CODE);
    const { url } = await fakeOllama("{}");
    server?.close();
    await expect(
      executeJob({
        rootDir: dir,
        scope: { kind: "workspace" },
        packIds: [],
        analyzers: { mode: "off" },
        semantic: { provider: "ollama", url },
      }),
    ).rejects.toThrow("not answering");
  });
});
