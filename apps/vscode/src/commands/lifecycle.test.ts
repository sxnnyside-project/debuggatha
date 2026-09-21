import { loadLedger } from "@debuggatha/engine";
import { afterAll, beforeEach, describe, expect, it } from "vitest";
import type { FindingTreeItem } from "../providers/findingsProvider.js";
import { makeRepo, removeRepos, seedLedger } from "../test/fixtures.js";
import { editorDiagnostics, harness } from "../test/harness.js";
import { mock, resetMock } from "../test/vscode-mock.js";
import { registerLifecycleCommands } from "./lifecycle.js";

afterAll(removeRepos);

const statusOf = (root: string, id: string) =>
  loadLedger(root).entries.find((entry) => entry.id === id)?.status;

function setup() {
  const { root } = makeRepo();
  seedLedger(root);
  const { services } = harness(root);
  services.refreshFromLedger();
  registerLifecycleCommands({ subscriptions: [] } as never, services, services.findings);

  const entries = loadLedger(root).entries;
  const first = entries[0];
  if (!first) throw new Error("the seeded review produced no findings");
  const fromTree = (command: string, id = first.id) =>
    (mock.commands.get(command) as (item: FindingTreeItem) => Promise<void>)({
      entry: { id },
    } as FindingTreeItem);
  const byId = (command: string, id = first.id) =>
    (mock.commands.get(command) as (id: string) => Promise<void>)(id);
  return { root, services, id: first.id, fromTree, byId, total: entries.length };
}

describe("finding lifecycle commands", () => {
  beforeEach(resetMock);

  it("registers a command for each transition, from the tree and from the editor", () => {
    setup();
    expect([...mock.commands.keys()].sort()).toEqual([
      "debuggatha.dismissFinding",
      "debuggatha.dismissFindingById",
      "debuggatha.reopenFinding",
      "debuggatha.resolveFinding",
      "debuggatha.resolveFindingById",
      "debuggatha.toggleResolvedFindings",
    ]);
  });

  it("resolves an open finding: ledger on disk, tree, editor diagnostics, and status bar all follow", async () => {
    const { root, id, fromTree, total } = setup();
    await fromTree("debuggatha.resolveFinding");

    expect(statusOf(root, id)).toBe("resolved");
    expect(mock.errors).toEqual([]);
    expect(editorDiagnostics().flat()).toHaveLength(total - 1);
    expect(mock.statusBarItems[0]?.text).toContain(`${total - 1}`);
  });

  it("does the same from the editor (a hover or the lightbulb), by id", async () => {
    const { root, id, byId, total } = setup();
    await byId("debuggatha.dismissFindingById");
    expect(statusOf(root, id)).toBe("dismissed");
    expect(editorDiagnostics().flat()).toHaveLength(total - 1);

    await byId("debuggatha.resolveFindingById", "no-such-id");
    expect(mock.errors).toEqual(["Finding not found in ledger."]);
  });

  it("reopens a resolved finding as reopened, the only state the lifecycle allows", async () => {
    const { root, id, fromTree, total } = setup();
    await fromTree("debuggatha.resolveFinding");
    await fromTree("debuggatha.reopenFinding");

    expect(mock.errors).toEqual([]);
    expect(statusOf(root, id)).toBe("reopened");
    expect(editorDiagnostics().flat()).toHaveLength(total);
  });

  it("reopens a dismissed finding too, and keeps it actionable", async () => {
    const { root, id, fromTree } = setup();
    await fromTree("debuggatha.dismissFinding");
    expect(statusOf(root, id)).toBe("dismissed");
    await fromTree("debuggatha.reopenFinding");
    expect(statusOf(root, id)).toBe("reopened");
    await fromTree("debuggatha.resolveFinding");
    expect(statusOf(root, id)).toBe("resolved");
  });

  it("refuses an illegal transition and leaves the ledger as it was", async () => {
    const { root, id, fromTree } = setup();
    await fromTree("debuggatha.dismissFinding");
    await fromTree("debuggatha.resolveFinding");

    expect(mock.errors).toHaveLength(1);
    expect(mock.errors[0]).toContain("Failed to update finding status");
    expect(statusOf(root, id)).toBe("dismissed");
  });

  it("does nothing when the status would not change", async () => {
    const { fromTree } = setup();
    await fromTree("debuggatha.resolveFinding");
    await fromTree("debuggatha.resolveFinding");
    expect(mock.errors).toEqual([]);
  });

  it("reports a finding that is not in the ledger", async () => {
    const { root, fromTree } = setup();
    await fromTree("debuggatha.resolveFinding", "no-such-id");
    expect(mock.errors).toEqual(["Finding not found in ledger."]);
    expect(loadLedger(root).entries.every((entry) => entry.status === "open")).toBe(true);
  });

  it("ignores a tree node that is not a finding", async () => {
    setup();
    const handler = mock.commands.get("debuggatha.resolveFinding") as (
      item: FindingTreeItem,
    ) => Promise<void>;
    await handler({} as FindingTreeItem);
    expect(mock.errors).toEqual([]);
  });

  it("does nothing without an open workspace", async () => {
    const { fromTree } = setup();
    mock.workspaceFolders = undefined;
    await fromTree("debuggatha.resolveFinding");
    expect(mock.errors).toEqual([]);
  });

  it("logs each change without the code it is about", async () => {
    const { fromTree } = setup();
    await fromTree("debuggatha.resolveFinding");
    expect(mock.outputLines.some((line) => line.includes("Finding status changed"))).toBe(true);
  });

  it("toggles the resolved-findings filter on the tree", () => {
    const { services } = setup();
    let fired = 0;
    services.findings.onDidChangeTreeData(() => {
      fired += 1;
    });
    (mock.commands.get("debuggatha.toggleResolvedFindings") as () => void)();
    expect(fired).toBe(1);
  });
});
