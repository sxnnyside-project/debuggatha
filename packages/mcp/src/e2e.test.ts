import { afterAll, beforeAll, describe, expect, it } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { request } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import {
  getDefaultEnvironment,
  StdioClientTransport,
} from "@modelcontextprotocol/sdk/client/stdio.js";
import { StreamableHTTPClientTransport } from "@modelcontextprotocol/sdk/client/streamableHttp.js";
import type { Transport } from "@modelcontextprotocol/sdk/shared/transport.js";
import {
  CreateMessageRequestSchema,
  ListRootsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { loadConfig } from "./config.js";
import { createDefaultDomainDeps } from "./domain-deps.js";
import { startHttpServer } from "./http.js";
import { createLogger } from "./logging.js";
import { createServer } from "./server.js";

const BIN = resolve(import.meta.dir, "bin.ts");
const cleanup: string[] = [];

/** A small TypeScript/Express repository with lines that real Review Packs flag. */
function makeRepo(
  source = 'var x: any = 1;\nif (x == 2) { eval("1"); }\nconst password = "hunter22222";\n',
): string {
  const dir = mkdtempSync(join(tmpdir(), "debuggatha-mcp-e2e-"));
  cleanup.push(dir);
  mkdirSync(join(dir, "src"));
  writeFileSync(
    join(dir, "package.json"),
    JSON.stringify({
      name: "fixture",
      dependencies: { express: "^4.0.0" },
      devDependencies: { typescript: "^5.0.0" },
    }),
  );
  writeFileSync(join(dir, "src", "bad.ts"), source);
  return dir;
}

async function connectStdio(env: Record<string, string> = {}, withRoots?: string): Promise<Client> {
  const client = new Client(
    { name: "e2e", version: "0" },
    withRoots ? { capabilities: { roots: {} } } : {},
  );
  if (withRoots) {
    client.setRequestHandler(ListRootsRequestSchema, async () => ({
      roots: [{ uri: pathToFileURL(withRoots).href, name: "workspace" }],
    }));
  }
  await client.connect(
    new StdioClientTransport({
      command: process.execPath,
      args: [BIN],
      env: { ...getDefaultEnvironment(), DEBUGGATHA_LOG_LEVEL: "error", ...env },
    }),
  );
  return client;
}

const text = (result: unknown): string =>
  ((result as { content: { text: string }[] }).content[0] as { text: string }).text;

afterAll(() => {
  for (const dir of cleanup) rmSync(dir, { recursive: true, force: true });
});

describe("stdio server (real client, real process)", () => {
  let repo: string;
  let client: Client;

  beforeAll(async () => {
    repo = makeRepo();
    client = await connectStdio({ DEBUGGATHA_REPOSITORY_ROOT: repo });
  });
  afterAll(async () => {
    await client.close();
  });

  it("advertises every tool with a title, an output schema, and honest annotations", async () => {
    const { tools } = await client.listTools();
    expect(tools.map((t) => t.name).sort()).toEqual([
      "create_baseline",
      "explain_finding",
      "get_finding",
      "list_analyzers",
      "list_findings",
      "repository_context",
      "repository_summary",
      "review_changes",
      "review_diff",
      "review_files",
      "review_workspace",
      "suppress_finding",
      "update_finding",
    ]);
    // Only tools that hide findings from later reviews are destructive.
    const hides = new Set(["create_baseline", "suppress_finding"]);
    for (const tool of tools) {
      expect(tool.title).toBeTruthy();
      expect(tool.outputSchema?.type).toBe("object");
      expect(tool.annotations?.destructiveHint).toBe(hides.has(tool.name));
      expect(tool.annotations?.openWorldHint).toBe(false);
    }
    const byName = new Map(tools.map((t) => [t.name, t]));
    for (const readOnly of [
      "list_analyzers",
      "list_findings",
      "get_finding",
      "explain_finding",
      "repository_context",
      "repository_summary",
    ]) {
      expect(byName.get(readOnly)?.annotations?.readOnlyHint).toBe(true);
    }
    for (const writes of [
      "review_diff",
      "review_files",
      "review_workspace",
      "review_changes",
      "update_finding",
      "create_baseline",
      "suppress_finding",
    ]) {
      expect(byName.get(writes)?.annotations?.readOnlyHint).toBe(false);
    }
    expect(JSON.stringify(byName.get("review_files")?.inputSchema)).not.toContain("depth");
  });

  it("reviews files with detected packs and returns structured findings", async () => {
    const result = await client.callTool({
      name: "review_files",
      arguments: { paths: [join(repo, "src", "bad.ts")] },
    });
    expect(result.isError).toBeFalsy();
    const out = result.structuredContent as {
      summary: { totalFindings: number; appliedPacks: { id: string }[] };
      findings: { severity: string; evidence: unknown[] }[];
      persisted: boolean;
    };
    expect(out.summary.totalFindings).toBeGreaterThan(0);
    expect(out.summary.appliedPacks.length).toBeGreaterThan(0);
    expect(out.findings.every((f) => f.evidence.length > 0)).toBe(true);
    expect(out.persisted).toBe(true);
    expect(JSON.parse(text(result))).toEqual(out);
    expect(existsSync(join(repo, ".debuggatha", "ledger.json"))).toBe(true);
  });

  it("walks a finding through the ledger: list, get, update", async () => {
    const listed = (
      await client.callTool({ name: "list_findings", arguments: { status: ["open"] } })
    ).structuredContent as { total: number; entries: { id: string; status: string }[] };
    expect(listed.total).toBeGreaterThan(0);
    const id = listed.entries[0]?.id as string;

    const fetched = (await client.callTool({ name: "get_finding", arguments: { findingId: id } }))
      .structuredContent as { entry: { id: string } };
    expect(fetched.entry.id).toBe(id);

    const updated = (
      await client.callTool({
        name: "update_finding",
        arguments: { findingId: id, status: "dismissed", comment: "accepted risk" },
      })
    ).structuredContent as { entry: { status: string } };
    expect(updated.entry.status).toBe("dismissed");

    const stillOpen = (
      await client.callTool({ name: "list_findings", arguments: { status: ["open"] } })
    ).structuredContent as { entries: { id: string }[] };
    expect(stillOpen.entries.some((e) => e.id === id)).toBe(false);
  });

  it("reviews read-only when persist is false", async () => {
    const fresh = makeRepo();
    const result = await client.callTool({
      name: "review_workspace",
      arguments: { rootDir: fresh, persist: false },
    });
    const out = result.structuredContent as {
      summary: { totalFindings: number };
      persisted: boolean;
    };
    expect(out.summary.totalFindings).toBeGreaterThan(0);
    expect(out.persisted).toBe(false);
    expect(existsSync(join(fresh, ".debuggatha"))).toBe(false);
  });

  it("summarizes the repository and detects its stack", async () => {
    const result = await client.callTool({ name: "repository_summary", arguments: {} });
    const out = result.structuredContent as { rootDir: string; capabilities: { id: string }[] };
    expect(out.rootDir).toBe(repo);
    expect(out.capabilities.map((c) => c.id)).toContain("express");
  });

  it("rejects bad input with a message naming the field", async () => {
    const result = await client
      .callTool({ name: "review_files", arguments: { paths: [] } })
      .catch((error: Error) => error);
    const message = result instanceof Error ? result.message : text(result);
    expect(message).toContain("paths");
  });

  it("reports an unknown finding without leaking filesystem paths", async () => {
    const result = await client.callTool({ name: "get_finding", arguments: { findingId: "nope" } });
    expect(result.isError).toBe(true);
    expect(text(result)).toContain("nope");
    expect(text(result)).not.toContain(repo);
  });

  it("serves resources for the default repository and for an explicit rootDir", async () => {
    const { resources } = await client.listResources();
    expect(resources.map((r) => r.uri)).toContain("debuggatha://ledger");

    const ledger = await client.readResource({ uri: "debuggatha://ledger" });
    expect(
      JSON.parse((ledger.contents[0] as { text: string }).text).entries.length,
    ).toBeGreaterThan(0);

    const other = makeRepo();
    const summary = await client.readResource({
      uri: `debuggatha://repository/summary?rootDir=${encodeURIComponent(other)}`,
    });
    expect(JSON.parse((summary.contents[0] as { text: string }).text).rootDir).toBe(other);
  });

  it("does not leak filesystem paths when a resource read fails", async () => {
    const missing = join(tmpdir(), "debuggatha-does-not-exist");
    const failure = await client
      .readResource({ uri: `debuggatha://ledger?rootDir=${encodeURIComponent(missing)}` })
      .then(() => undefined)
      .catch((error: Error) => error);
    if (failure) expect(failure.message).not.toContain(missing);
  });

  it("offers review and triage prompts that point at real tools", async () => {
    const { prompts } = await client.listPrompts();
    expect(prompts.map((p) => p.name).sort()).toEqual([
      "fix-findings",
      "review-flow",
      "triage-findings",
    ]);
    const prompt = await client.getPrompt({ name: "review-flow", arguments: {} });
    const first = prompt.messages[0] as { content: { text: string } };
    for (const tool of ["review_changes", "explain_finding", "suppress_finding"]) {
      expect(first.content.text).toContain(tool);
    }
    expect(first.content.text).toContain("Never accept a finding just to make a review quiet");
  });
});

describe("repository root resolution", () => {
  it("falls back to the workspace root the client shares", async () => {
    const repo = makeRepo();
    const client = await connectStdio({}, repo);
    try {
      const result = await client.callTool({ name: "repository_summary", arguments: {} });
      expect((result.structuredContent as { rootDir: string }).rootDir).toBe(repo);
    } finally {
      await client.close();
    }
  });

  it("explains every way to provide a root when none is available", async () => {
    const client = await connectStdio();
    try {
      const result = await client.callTool({ name: "repository_summary", arguments: {} });
      expect(result.isError).toBe(true);
      expect(text(result)).toContain("DEBUGGATHA_REPOSITORY_ROOT");
      expect(text(result)).toContain("root");
    } finally {
      await client.close();
    }
  });
});

describe("trust, through the tool interface", () => {
  it("returns findings with columns, confidence, and a fix an agent can act on", async () => {
    const repo = makeRepo("  var count = 0;\n");
    const client = await connectStdio({ DEBUGGATHA_REPOSITORY_ROOT: repo });
    try {
      const result = await client.callTool({
        name: "review_workspace",
        arguments: { persist: false },
      });
      const out = result.structuredContent as {
        findings: {
          confidence: string;
          locations: { columns: { start: number; end: number } }[];
          recommendations: { summary: string; example: { before: string; after: string } }[];
        }[];
      };
      const [finding] = out.findings;
      expect(finding?.locations[0]?.columns).toEqual({ start: 3, end: 8 });
      expect(finding?.confidence).toBe("high");
      expect(finding?.recommendations[0]?.summary).toContain("const");
      expect(finding?.recommendations[0]?.example.after).toBe("let total = 0;");
    } finally {
      await client.close();
    }
  });

  it("does not review dependency or generated code", async () => {
    const repo = makeRepo();
    mkdirSync(join(repo, "vendor"));
    writeFileSync(join(repo, "vendor", "lib.js"), "var v = eval(x);\n");
    const client = await connectStdio({ DEBUGGATHA_REPOSITORY_ROOT: repo });
    try {
      const out = (
        await client.callTool({ name: "review_workspace", arguments: { persist: false } })
      ).structuredContent as { findings: { locations: { file: string }[] }[] };
      const files = new Set(out.findings.map((f) => f.locations[0]?.file));
      expect([...files]).toEqual(["src/bad.ts"]);
    } finally {
      await client.close();
    }
  });

  it("honors an inline debuggatha-ignore comment and reports what it hid", async () => {
    const repo = makeRepo("run(eval(x)); // debuggatha-ignore no-eval -- sandboxed\n");
    const client = await connectStdio({ DEBUGGATHA_REPOSITORY_ROOT: repo });
    try {
      const out = (
        await client.callTool({
          name: "review_files",
          arguments: { paths: [join(repo, "src", "bad.ts")], persist: false },
        })
      ).structuredContent as {
        summary: { totalFindings: number };
        suppressedInline: { file: string; line: number; ruleId: string; reason: string }[];
      };
      expect(out.summary.totalFindings).toBe(0);
      expect(out.suppressedInline).toEqual([
        { file: "src/bad.ts", line: 1, ruleId: "no-eval", reason: "sandboxed" },
      ]);
    } finally {
      await client.close();
    }
  });

  it("adoption mode: create_baseline quiets what exists and reports only what is new", async () => {
    const repo = makeRepo("var old = 1;\n");
    const client = await connectStdio({ DEBUGGATHA_REPOSITORY_ROOT: repo });
    const review = async (args: Record<string, unknown> = {}) =>
      (await client.callTool({ name: "review_workspace", arguments: { persist: false, ...args } }))
        .structuredContent as {
        summary: { totalFindings: number };
        baselined: number;
        findings: { locations: { file: string }[] }[];
      };
    try {
      const created = (await client.callTool({ name: "create_baseline", arguments: {} }))
        .structuredContent as { file: string; findings: number; files: number };
      expect(created).toMatchObject({ file: ".debuggatha/baseline.json", findings: 1, files: 1 });

      const quiet = await review();
      expect(quiet.summary.totalFindings).toBe(0);
      expect(quiet.baselined).toBe(1);

      writeFileSync(join(repo, "src", "new.ts"), "var fresh = 1;\n");
      const later = await review();
      expect(later.findings.map((f) => f.locations[0]?.file)).toEqual(["src/new.ts"]);

      const everything = await review({ includeBaselined: true });
      expect(everything.summary.totalFindings).toBe(2);
      expect(everything.baselined).toBe(0);
    } finally {
      await client.close();
    }
  });

  it("never puts a secret it found in the ledger it writes", async () => {
    const secret = "sk_live_1234567890abcdef";
    const repo = makeRepo(`const key = "${secret}";\n`);
    const client = await connectStdio({ DEBUGGATHA_REPOSITORY_ROOT: repo });
    try {
      const result = await client.callTool({ name: "review_workspace", arguments: {} });
      expect(text(result)).not.toContain(secret);
      expect(readFileSync(join(repo, ".debuggatha", "ledger.json"), "utf8")).not.toContain(secret);
    } finally {
      await client.close();
    }
  });
});

describe("streamable HTTP server", () => {
  const server = createServer;
  let port: number;
  let close: () => void;

  beforeAll(async () => {
    const config = loadConfig({});
    const logger = createLogger({ minLevel: "error" });
    const deps = createDefaultDomainDeps();
    const http = await startHttpServer({
      host: "127.0.0.1",
      port: 0,
      logger,
      createServer: () => server(deps, config, logger, undefined),
    });
    port = (http.address() as AddressInfo).port;
    close = () => http.close();
  });
  afterAll(() => close());

  it("serves tools to a real HTTP client", async () => {
    const repo = makeRepo();
    const client = new Client({ name: "e2e-http", version: "0" });
    await client.connect(
      new StreamableHTTPClientTransport(new URL(`http://127.0.0.1:${port}/mcp`)) as Transport,
    );
    try {
      const { tools } = await client.listTools();
      expect(tools.length).toBe(13);
      const result = await client.callTool({
        name: "review_workspace",
        arguments: { rootDir: repo, persist: false },
      });
      expect(
        (result.structuredContent as { summary: { totalFindings: number } }).summary.totalFindings,
      ).toBeGreaterThan(0);
    } finally {
      await client.close();
    }
  });

  const raw = (method: string, path: string, headers: Record<string, string> = {}) =>
    new Promise<number>((resolveStatus, reject) => {
      const req = request({ host: "127.0.0.1", port, method, path, headers }, (res) => {
        res.resume();
        resolveStatus(res.statusCode ?? 0);
      });
      req.on("error", reject);
      req.end();
    });

  it("refuses non-POST requests, unknown paths, and foreign Host headers", async () => {
    expect(await raw("GET", "/mcp")).toBe(405);
    expect(await raw("POST", "/other")).toBe(404);
    expect(await raw("POST", "/mcp", { host: "evil.example" })).toBe(403);
  });
});

describe("bin", () => {
  it("prints its version and usage without starting a server", () => {
    const version = Bun.spawnSync([process.execPath, BIN, "--version"]);
    expect(version.stdout.toString().trim()).toMatch(/^\d+\.\d+\.\d+$/);
    const help = Bun.spawnSync([process.execPath, BIN, "--help"]);
    expect(help.stdout.toString()).toContain("--http");
  });
});

describe("external analyzers over MCP", () => {
  /** A `ruff` on its own PATH that reports one finding in `app.py`, and a marker showing it ran. */
  function withTools(repo: string, tools: ("ruff" | "eslint")[]) {
    const bin = mkdtempSync(join(tmpdir(), "debuggatha-mcp-bin-"));
    cleanup.push(bin);
    const ruff = JSON.stringify([
      {
        code: "F401",
        message: "`os` imported but unused",
        filename: join(repo, "app.py"),
        location: { row: 1, column: 8 },
        end_location: { row: 1, column: 10 },
      },
    ]);
    const bodies = {
      ruff: `if [ "$1" = "--version" ]; then echo "ruff 0.15.1"; exit 0; fi\necho '${ruff}'`,
      eslint: `if [ "$1" = "--version" ]; then echo "v9.0.0"; exit 0; fi\ntouch "${join(repo, "eslint-ran")}"\necho '[]'`,
    };
    for (const tool of tools) {
      writeFileSync(join(bin, tool), `#!/bin/sh\n${bodies[tool]}\n`);
      Bun.spawnSync(["chmod", "755", join(bin, tool)]);
    }
    return `${bin}:${process.env.PATH ?? ""}`;
  }

  function pythonRepo() {
    const repo = makeRepo("export const a = 1;\n");
    writeFileSync(join(repo, "app.py"), "import os\n");
    writeFileSync(join(repo, "eslint.config.js"), "export default [];\n");
    return repo;
  }

  const structured = (result: unknown) =>
    (result as { structuredContent: Record<string, unknown> }).structuredContent;

  it("runs an installed analyzer in a review and reports the tool, version, and license", async () => {
    const repo = pythonRepo();
    const client = await connectStdio({
      DEBUGGATHA_REPOSITORY_ROOT: repo,
      PATH: withTools(repo, ["ruff"]),
    });
    try {
      const review = structured(await client.callTool({ name: "review_workspace", arguments: {} }));
      const findings = review.findings as { evidence: Record<string, unknown>[] }[];
      const external = findings
        .flatMap((f) => f.evidence)
        .find((e) => e.kind === "external-analyzer");
      expect(external).toMatchObject({
        tool: "ruff",
        ruleId: "F401",
        version: "0.15.1",
        license: "MIT",
      });
      expect(
        (review.analyzers as { id: string; status: string }[]).find((a) => a.id === "ruff"),
      ).toMatchObject({
        status: "ran",
      });

      const off = structured(
        await client.callTool({ name: "review_workspace", arguments: { analyzers: false } }),
      );
      expect(off.analyzers).toEqual([]);
    } finally {
      await client.close();
    }
  });

  it("an agent cannot enable a tool that runs project code; the operator can", async () => {
    const repo = pythonRepo();
    const path = withTools(repo, ["eslint"]);

    const guarded = await connectStdio({ DEBUGGATHA_REPOSITORY_ROOT: repo, PATH: path });
    try {
      // Nothing a tool call sends can turn it on: `analyzers` only accepts a boolean.
      const attempt = await guarded
        .callTool({ name: "review_workspace", arguments: { analyzers: ["eslint"] } })
        .catch((error: unknown) => error);
      expect(JSON.stringify(attempt)).not.toContain('"status":"ran"');
      const review = structured(
        await guarded.callTool({ name: "review_workspace", arguments: {} }),
      );
      expect(existsSync(join(repo, "eslint-ran"))).toBe(false);
      const skipped = (review.analyzers as { id: string; status: string; reason?: string }[]).find(
        (a) => a.id === "eslint",
      );
      expect(skipped?.status).toBe("skipped");
      expect(skipped?.reason).toContain("DEBUGGATHA_ANALYZERS=eslint");
    } finally {
      await guarded.close();
    }

    const trusted = await connectStdio({
      DEBUGGATHA_REPOSITORY_ROOT: repo,
      PATH: path,
      DEBUGGATHA_ANALYZERS: "eslint",
    });
    try {
      await trusted.callTool({ name: "review_workspace", arguments: {} });
      expect(existsSync(join(repo, "eslint-ran"))).toBe(true);
    } finally {
      await trusted.close();
    }
  });

  it("list_analyzers says what is installed and what would run", async () => {
    const repo = pythonRepo();
    const client = await connectStdio({
      DEBUGGATHA_REPOSITORY_ROOT: repo,
      PATH: withTools(repo, ["ruff", "eslint"]),
    });
    try {
      const { analyzers } = structured(
        await client.callTool({ name: "list_analyzers", arguments: {} }),
      ) as {
        analyzers: {
          id: string;
          installed: boolean;
          willRun: boolean;
          license: string;
          reason?: string;
        }[];
      };
      const byId = new Map(analyzers.map((a) => [a.id, a]));
      expect(byId.get("ruff")).toMatchObject({ installed: true, willRun: true, license: "MIT" });
      expect(byId.get("eslint")).toMatchObject({ installed: true, willRun: false });
      expect(byId.get("ktlint")).toMatchObject({ installed: false, willRun: false });
    } finally {
      await client.close();
    }
  });

  it("DEBUGGATHA_ANALYZERS=off runs no analyzer at all", async () => {
    const repo = pythonRepo();
    const client = await connectStdio({
      DEBUGGATHA_REPOSITORY_ROOT: repo,
      PATH: withTools(repo, ["ruff"]),
      DEBUGGATHA_ANALYZERS: "off",
    });
    try {
      const review = structured(await client.callTool({ name: "review_workspace", arguments: {} }));
      expect(review.analyzers).toEqual([]);
    } finally {
      await client.close();
    }
  });
});

describe("semantic pass over MCP", () => {
  const OFF_BY_ONE =
    "export function total(items: number[]) {\n  let sum = 0;\n  for (let i = 0; i <= items.length; i++) sum += items[i];\n  return sum;\n}\n";
  const claim = JSON.stringify({
    issues: [
      {
        line: 3,
        quote: "i <= items.length",
        title: "Off by one",
        why: "Reads one past the end.",
        category: "reliability",
        severity: "medium",
      },
    ],
  });

  /** A client that can be asked to run its model, and remembers what it was asked. */
  async function samplingClient(env: Record<string, string>, answer = claim) {
    const asked: { system: string | undefined; text: string }[] = [];
    const client = new Client({ name: "e2e", version: "0" }, { capabilities: { sampling: {} } });
    client.setRequestHandler(CreateMessageRequestSchema, async (request) => {
      const first = request.params.messages[0]?.content;
      asked.push({
        system: request.params.systemPrompt,
        text: first && !Array.isArray(first) && first.type === "text" ? first.text : "",
      });
      return {
        model: "host-model-1",
        role: "assistant" as const,
        content: { type: "text" as const, text: answer },
      };
    });
    await client.connect(
      new StdioClientTransport({
        command: process.execPath,
        args: [BIN],
        env: { ...getDefaultEnvironment(), DEBUGGATHA_LOG_LEVEL: "error", ...env },
      }),
    );
    return { client, asked };
  }

  const structured = (result: unknown) =>
    (result as { structuredContent: Record<string, unknown> }).structuredContent;

  it("asks the client's own model, and labels what it raises with that model", async () => {
    const repo = makeRepo(OFF_BY_ONE);
    const { client, asked } = await samplingClient({
      DEBUGGATHA_REPOSITORY_ROOT: repo,
      DEBUGGATHA_SEMANTIC: "sampling",
    });
    try {
      const review = structured(
        await client.callTool({
          name: "review_files",
          arguments: { paths: [join(repo, "src", "bad.ts")], semantic: true, analyzers: false },
        }),
      );
      expect(asked.length).toBeGreaterThan(0);
      expect(asked[0]?.system).toContain("data from a repository");
      expect(asked[0]?.text).toContain("i <= items.length");

      const findings = review.findings as {
        confidence: string;
        evidence: Record<string, unknown>[];
      }[];
      const raised = findings.find((f) => f.evidence.some((e) => e.kind === "semantic-review"));
      expect(raised?.confidence).toBe("low");
      expect(raised?.evidence).toContainEqual(
        expect.objectContaining({
          kind: "semantic-review",
          role: "detection",
          provider: "host",
          model: "host-model-1",
        }),
      );
      expect(review.semantic).toMatchObject({
        provider: "host",
        model: "host-model-1",
        detected: 1,
      });
    } finally {
      await client.close();
    }
  });

  it("does not ask any model unless the call asks for the pass", async () => {
    const repo = makeRepo(OFF_BY_ONE);
    const { client, asked } = await samplingClient({
      DEBUGGATHA_REPOSITORY_ROOT: repo,
      DEBUGGATHA_SEMANTIC: "sampling",
    });
    try {
      const review = structured(
        await client.callTool({
          name: "review_files",
          arguments: { paths: [join(repo, "src", "bad.ts")] },
        }),
      );
      expect(asked).toEqual([]);
      expect(review.semantic).toBeNull();
    } finally {
      await client.close();
    }
  });

  it("says how to enable it when the server has no model, and does not review", async () => {
    const repo = makeRepo(OFF_BY_ONE);
    const { client, asked } = await samplingClient({ DEBUGGATHA_REPOSITORY_ROOT: repo });
    try {
      const result = await client.callTool({
        name: "review_files",
        arguments: { paths: [join(repo, "src", "bad.ts")], semantic: true },
      });
      expect(result.isError).toBe(true);
      expect(text(result)).toContain("DEBUGGATHA_SEMANTIC");
      expect(asked).toEqual([]);
    } finally {
      await client.close();
    }
  });

  it("explains a client that cannot sample", async () => {
    const repo = makeRepo(OFF_BY_ONE);
    const client = await connectStdio({
      DEBUGGATHA_REPOSITORY_ROOT: repo,
      DEBUGGATHA_SEMANTIC: "sampling",
    });
    try {
      const result = await client.callTool({
        name: "review_files",
        arguments: { paths: [join(repo, "src", "bad.ts")], semantic: true },
      });
      expect(result.isError).toBe(true);
      expect(text(result)).toContain("does not support sampling");
    } finally {
      await client.close();
    }
  });

  it("a model that lies about the code adds nothing", async () => {
    const repo = makeRepo(OFF_BY_ONE);
    const invented = JSON.stringify({
      issues: [
        {
          line: 3,
          quote: "db.query(userInput)",
          title: "SQL injection",
          why: "x",
          category: "security",
          severity: "medium",
        },
      ],
    });
    const { client } = await samplingClient(
      { DEBUGGATHA_REPOSITORY_ROOT: repo, DEBUGGATHA_SEMANTIC: "sampling" },
      invented,
    );
    try {
      const review = structured(
        await client.callTool({
          name: "review_files",
          arguments: { paths: [join(repo, "src", "bad.ts")], semantic: true, analyzers: false },
        }),
      );
      expect(review.semantic).toMatchObject({ detected: 0, rejected: 1 });
    } finally {
      await client.close();
    }
  });
});
