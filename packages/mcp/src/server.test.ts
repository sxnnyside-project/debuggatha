import { beforeEach, describe, expect, it, vi } from "bun:test";
import type { Server } from "@modelcontextprotocol/sdk/server/index.js";
import type { Transport } from "@modelcontextprotocol/sdk/shared/transport.js";
import type { JSONRPCMessage } from "@modelcontextprotocol/sdk/types.js";
import { loadConfig } from "./config.js";
import type { DomainDeps } from "./domain-deps.js";
import { createLogger } from "./logging.js";
import { createServer } from "./server.js";

class InMemoryTransport implements Transport {
  onclose?: () => void;
  onerror?: (error: Error) => void;
  onmessage?: (message: JSONRPCMessage) => void;

  sent: JSONRPCMessage[] = [];

  async start(): Promise<void> {}
  async close(): Promise<void> {
    this.onclose?.();
  }
  async send(message: JSONRPCMessage): Promise<void> {
    this.sent.push(message);
  }
}

async function sendRequest(
  transport: InMemoryTransport,
  id: number,
  method: string,
  params?: any,
): Promise<any> {
  const promise = new Promise((resolve) => {
    const originalSend = transport.send.bind(transport);
    transport.send = async (msg) => {
      await originalSend(msg);
      if ("id" in msg && msg.id === id) {
        resolve(msg);
      }
    };
  });

  await transport.onmessage?.({
    jsonrpc: "2.0",
    id,
    method,
    params,
  });

  return promise;
}

async function initializeServer(server: Server, transport: InMemoryTransport) {
  await server.connect(transport);

  // Send initialize request
  const initResponse = await sendRequest(transport, 1, "initialize", {
    protocolVersion: "2024-11-05",
    capabilities: {},
    clientInfo: { name: "test-client", version: "1.0.0" },
  });

  // Send initialized notification
  await transport.onmessage?.({
    jsonrpc: "2.0",
    method: "notifications/initialized",
  });

  transport.sent = [];
  return initResponse;
}

describe("MCP Server", () => {
  let mockDeps: DomainDeps;
  const config = loadConfig({ DEBUGGATHA_REPOSITORY_ROOT: "/test-repo" });
  let logLines: string[] = [];
  const logger = createLogger({ minLevel: "debug", write: (l) => logLines.push(l) });

  beforeEach(() => {
    logLines = [];
    mockDeps = {
      buildRepositoryContext: vi.fn().mockReturnValue({
        rootDir: "/test-repo",
        generatedAt: "2026-07-10T12:00:00Z",
        hasGit: true,
        stack: { profile: "typescript" },
        dependencies: { profile: {} },
        criteria: { profile: {} },
        understanding: {},
      }),
      createReviewRequest: vi.fn().mockReturnValue({
        id: "req-1",
        scope: { kind: "workspace" },
        depth: "full",
        requestedAt: "2026-07-10T12:00:00Z",
        requestedPackIds: [],
        requestedPolicyId: undefined,
      }),
      assemblePolicy: vi.fn().mockReturnValue({
        id: "policy-1",
        packRefs: [],
      }),
      createReviewSession: vi.fn().mockReturnValue({
        id: "session-1",
        status: "idle",
        request: {},
        repositoryContext: {},
      }),
      transitionSession: vi.fn().mockImplementation((session, status) => ({
        ...session,
        status,
      })),
      createReviewResult: vi.fn().mockReturnValue({
        session: { id: "session-1", status: "completed" },
        findings: [],
      }),
      reviewDiff: vi.fn().mockReturnValue([]),
      reviewArchitecture: vi.fn().mockReturnValue([]),
      reviewFiles: vi.fn().mockImplementation(() => {
        throw new Error("No Review Skill implements file-scoped review yet");
      }),
      synchronizeReviewResult: vi.fn().mockReturnValue({
        ledger: { repositoryRoot: "/test-repo", entries: [], schemaVersion: 1 },
        report: { added: 0, resolved: 0, active: 0 },
      }),
      loadLedger: vi.fn().mockReturnValue({
        repositoryRoot: "/test-repo",
        entries: [
          {
            id: "finding-1",
            fingerprint: { file: "src/main.ts", ruleId: "rule-1" },
            status: "open",
            history: [{ timestamp: "2026-07-10T12:00:00Z", newState: "open" }],
            latestFinding: { category: "security", severity: "high" },
          },
        ],
        schemaVersion: 1,
      }),
      saveLedger: vi.fn(),
      listEntries: vi.fn().mockImplementation((ledger, _filter) => ledger.entries),
      getEntry: vi
        .fn()
        .mockImplementation((ledger, id) => ledger.entries.find((e: any) => e.id === id)),
      updateFindingStatus: vi.fn().mockImplementation((ledger, id, status) => ({
        ...ledger,
        entries: ledger.entries.map((e: any) => (e.id === id ? { ...e, status } : e)),
      })),
      summarizeLedger: vi.fn().mockReturnValue({
        countsByStatus: { open: 1, acknowledged: 0, resolved: 0, dismissed: 0, reopened: 0 },
        activeBySeverity: { info: 0, low: 0, medium: 0, high: 1, critical: 0 },
        activeByCategory: { security: 1 },
      }),
      summarizeCapabilities: vi.fn().mockReturnValue("Languages: TypeScript"),
      loadMemoryStore: vi.fn().mockReturnValue({
        schemaVersion: 1,
        repositoryRoot: "/repo",
        items: [],
        createdAt: "t",
        updatedAt: "t",
      }),
      filterSuppressedFindings: vi
        .fn()
        .mockImplementation((findings) => ({ findings, suppressed: [] })),
    };
  });

  it("advertises server capabilities during initialization", async () => {
    const server = createServer(mockDeps, config, logger, undefined);
    const transport = new InMemoryTransport();

    const response = await initializeServer(server, transport);
    expect(response.result).toMatchObject({
      protocolVersion: "2024-11-05",
      capabilities: {
        tools: {},
        resources: {},
        prompts: {},
      },
      serverInfo: {
        name: "debuggatha",
        version: "0.1.0",
      },
    });
  });

  it("lists all registered tools", async () => {
    const server = createServer(mockDeps, config, logger, undefined);
    const transport = new InMemoryTransport();
    await initializeServer(server, transport);

    const response = await sendRequest(transport, 2, "tools/list");
    const tools = response.result.tools;

    expect(tools).toHaveLength(8);
    const toolNames = tools.map((t: any) => t.name);
    expect(toolNames).toContain("review_diff");
    expect(toolNames).toContain("review_files");
    expect(toolNames).toContain("review_workspace");
    expect(toolNames).toContain("list_findings");
    expect(toolNames).toContain("get_finding");
    expect(toolNames).toContain("update_finding");
    expect(toolNames).toContain("repository_context");
    expect(toolNames).toContain("repository_summary");
  });

  it("routes tools/call to domain dependencies and logs execution", async () => {
    const server = createServer(mockDeps, config, logger, undefined);
    const transport = new InMemoryTransport();
    await initializeServer(server, transport);

    const response = await sendRequest(transport, 2, "tools/call", {
      name: "repository_context",
      arguments: {},
    });

    expect(response.result.isError).toBeUndefined();
    expect(JSON.parse(response.result.content[0].text)).toHaveProperty("rootDir", "/test-repo");
    expect(mockDeps.buildRepositoryContext).toHaveBeenCalledWith("/test-repo", {
      cache: undefined,
    });

    // Check logs
    const parsedLogs = logLines.map((l) => JSON.parse(l));
    expect(
      parsedLogs.some(
        (l) => l.event === "tool.execution.start" && l.data.tool === "repository_context",
      ),
    ).toBe(true);
    expect(
      parsedLogs.some(
        (l) => l.event === "tool.execution.success" && l.data.tool === "repository_context",
      ),
    ).toBe(true);
  });

  it("correctly maps a review_workspace call to runReview pipeline", async () => {
    const server = createServer(mockDeps, config, logger, undefined);
    const transport = new InMemoryTransport();
    await initializeServer(server, transport);

    const response = await sendRequest(transport, 2, "tools/call", {
      name: "review_workspace",
      arguments: {
        depth: "architectural",
      },
    });

    expect(response.result.isError).toBeUndefined();
    expect(mockDeps.buildRepositoryContext).toHaveBeenCalled();
    expect(mockDeps.createReviewRequest).toHaveBeenCalledWith(
      expect.objectContaining({
        scope: { kind: "workspace" },
        depth: "architectural",
      }),
    );
    expect(mockDeps.reviewArchitecture).toHaveBeenCalled();
    expect(mockDeps.synchronizeReviewResult).toHaveBeenCalled();
    expect(mockDeps.saveLedger).toHaveBeenCalled();
  });

  it("gracefully maps and reports expected file-scoped review errors", async () => {
    const server = createServer(mockDeps, config, logger, undefined);
    const transport = new InMemoryTransport();
    await initializeServer(server, transport);

    const response = await sendRequest(transport, 2, "tools/call", {
      name: "review_files",
      arguments: {
        paths: ["src/main.ts"],
      },
    });

    expect(response.result.isError).toBe(true);
    expect(response.result.content[0].text).toContain(
      "No Review Skill implements file-scoped review yet",
    );
  });

  it("safely updates a finding and saves the ledger", async () => {
    const server = createServer(mockDeps, config, logger, undefined);
    const transport = new InMemoryTransport();
    await initializeServer(server, transport);

    const response = await sendRequest(transport, 2, "tools/call", {
      name: "update_finding",
      arguments: {
        findingId: "finding-1",
        status: "acknowledged",
        comment: "Legitimate finding",
      },
    });

    expect(response.result.isError).toBeUndefined();
    const data = JSON.parse(response.result.content[0].text);
    expect(data.status).toBe("acknowledged");
    expect(mockDeps.updateFindingStatus).toHaveBeenCalledWith(
      expect.any(Object),
      "finding-1",
      "acknowledged",
      { kind: "manual", actor: "mcp-server" },
      "Legitimate finding",
    );
    expect(mockDeps.saveLedger).toHaveBeenCalled();
  });

  it("lists, reads, and routes repository resources", async () => {
    const server = createServer(mockDeps, config, logger, undefined);
    const transport = new InMemoryTransport();
    await initializeServer(server, transport);

    // List resources
    const listRes = await sendRequest(transport, 2, "resources/list");
    expect(listRes.result.resources).toHaveLength(4);
    expect(listRes.result.resources.map((r: any) => r.uri)).toContain("debuggatha://ledger");

    // Read resource
    const readRes = await sendRequest(transport, 3, "resources/read", {
      uri: "debuggatha://ledger?rootDir=/test-repo",
    });
    expect(readRes.result.contents[0].uri).toBe("debuggatha://ledger?rootDir=/test-repo");
    const parsedLedger = JSON.parse(readRes.result.contents[0].text);
    expect(parsedLedger.repositoryRoot).toBe("/test-repo");
  });

  it("supports review and triage prompts", async () => {
    const server = createServer(mockDeps, config, logger, undefined);
    const transport = new InMemoryTransport();
    await initializeServer(server, transport);

    // List prompts
    const listPrompts = await sendRequest(transport, 2, "prompts/list");
    expect(listPrompts.result.prompts).toHaveLength(2);
    expect(listPrompts.result.prompts.map((p: any) => p.name)).toContain("review-flow");

    // Get prompt
    const getPrompt = await sendRequest(transport, 3, "prompts/get", {
      name: "review-flow",
      arguments: { rootDir: "/test-repo", depth: "quick" },
    });
    expect(getPrompt.result.messages[0].content.text).toContain(
      'Please run a review on the workspace in "/test-repo" with depth "quick"',
    );
  });
});
