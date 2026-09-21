import type { LedgerEntry } from "@debuggatha/engine";
import { beforeEach, describe, expect, it } from "vitest";
import { resetMock } from "../test/vscode-mock.js";
import { FindingsProvider, type FindingTreeItem } from "./findingsProvider.js";

type Status = LedgerEntry["status"];

function entry(id: string, status: Status, file = "/repo/a.ts", severity = "high"): LedgerEntry {
  return {
    id,
    status,
    latestFinding: {
      id: `finding-${id}`,
      title: `Title ${id}`,
      severity,
      locations: [{ file, lines: { start: 1, end: 1 } }],
    },
  } as unknown as LedgerEntry;
}

const children = async (provider: FindingsProvider, parent?: FindingTreeItem) =>
  provider.getChildren(parent);

describe("FindingsProvider", () => {
  let provider: FindingsProvider;

  beforeEach(() => {
    resetMock();
    provider = new FindingsProvider();
  });

  it("shows every active finding by default, including acknowledged and reopened ones", async () => {
    provider.refresh([
      entry("1", "open"),
      entry("2", "acknowledged"),
      entry("3", "reopened"),
      entry("4", "resolved"),
      entry("5", "dismissed"),
    ]);
    const [file] = await children(provider);
    const shown = await children(provider, file);
    expect(shown.map((item) => item.entry?.id)).toEqual(["1", "2", "3"]);
  });

  it("reveals closed findings when resolved ones are toggled on, and hides them again", async () => {
    provider.refresh([entry("1", "open"), entry("2", "resolved"), entry("3", "dismissed")]);
    provider.toggleResolved();
    const [file] = await children(provider);
    expect((await children(provider, file)).map((item) => item.entry?.id)).toEqual(["1", "2", "3"]);
    provider.toggleResolved();
    expect((await children(provider, file)).map((item) => item.entry?.id)).toEqual(["1"]);
  });

  it("groups findings under one expanded node per file", async () => {
    provider.refresh([entry("1", "open", "/repo/a.ts"), entry("2", "open", "/repo/b.ts")]);
    const files = await children(provider);
    expect(files.map((item) => item.label)).toEqual(["/repo/a.ts", "/repo/b.ts"]);
    expect(files.every((item) => item.contextValue === "file")).toBe(true);
  });

  it("puts findings without a location under a Workspace node", async () => {
    const homeless = entry("1", "open");
    (homeless.latestFinding as { locations: unknown[] }).locations = [];
    provider.refresh([homeless]);
    const [node] = await children(provider);
    expect(node?.label).toBe("Workspace");
    expect(node?.contextValue).toBe("project");
  });

  it("labels a finding with its status as context and opens it on click", async () => {
    provider.refresh([entry("1", "acknowledged")]);
    const [file] = await children(provider);
    const [item] = await children(provider, file);
    expect(item?.contextValue).toBe("finding-acknowledged");
    expect(item?.description).toBe("high • acknowledged");
    expect(item?.command?.command).toBe("debuggatha.openFinding");
    expect(item?.command?.arguments?.[0]).toMatchObject({ id: "finding-1" });
  });

  it("does not call an open finding resolved", async () => {
    provider.refresh([entry("1", "open")]);
    const [file] = await children(provider);
    const [item] = await children(provider, file);
    expect(item?.description).toBe("high • open");
  });

  it("returns nothing beneath a finding and notifies listeners when refreshed", async () => {
    let fired = 0;
    provider.onDidChangeTreeData(() => {
      fired += 1;
    });
    provider.refresh([entry("1", "open")]);
    provider.toggleResolved();
    expect(fired).toBe(2);
    const [file] = await children(provider);
    const [item] = await children(provider, file);
    expect(await children(provider, item)).toEqual([]);
  });
});
