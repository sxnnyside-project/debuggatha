import { readFileSync } from "node:fs";
import { join } from "node:path";
import { canTransitionFinding, type FindingLifecycleStatus } from "@debuggatha/engine";
import { afterAll, beforeEach, describe, expect, it } from "vitest";
import { activate, deactivate } from "./index.js";
import { makeRepo, removeRepos, seedLedger } from "./test/fixtures.js";
import { mock, openWorkspace, resetMock } from "./test/vscode-mock.js";

afterAll(removeRepos);

interface Manifest {
  contributes: {
    commands: { command: string }[];
    menus: { "view/item/context": { command: string; when: string }[] };
  };
}

const manifest = JSON.parse(
  readFileSync(join(import.meta.dirname, "..", "package.json"), "utf8"),
) as Manifest;

const context = () => ({ subscriptions: [] as unknown[] }) as never;

describe("activation", () => {
  beforeEach(resetMock);

  it("registers exactly the commands the manifest declares", () => {
    activate(context());
    const declared = manifest.contributes.commands.map((c) => c.command).sort();
    expect([...mock.commands.keys()].sort()).toEqual(declared);
  });

  it("still registers everything when no folder is open", () => {
    activate(context());
    expect(mock.workspaceFolders).toBeUndefined();
    expect(mock.commands.size).toBeGreaterThan(0);
  });

  it("restores the previous session: findings in the editor and in the tree", () => {
    const { root, badFile } = makeRepo();
    seedLedger(root);
    resetMock();
    openWorkspace(root);

    activate(context());

    expect(mock.collections.get("debuggatha")?.entries.has(badFile)).toBe(true);
  });

  it("copes with a workspace that has never been reviewed", () => {
    const { root } = makeRepo();
    openWorkspace(root);
    expect(() => activate(context())).not.toThrow();
    expect(mock.collections.get("debuggatha")?.entries.size).toBe(0);
  });

  it("clears diagnostics on deactivation", () => {
    const { root } = makeRepo();
    seedLedger(root);
    resetMock();
    openWorkspace(root);
    activate(context());
    deactivate();
    expect(mock.collections.get("debuggatha")?.entries.size).toBe(0);
  });
});

describe("finding context menu", () => {
  const STATUSES: FindingLifecycleStatus[] = [
    "open",
    "acknowledged",
    "resolved",
    "dismissed",
    "reopened",
  ];
  const TARGET: Record<string, FindingLifecycleStatus> = {
    "debuggatha.resolveFinding": "resolved",
    "debuggatha.dismissFinding": "dismissed",
    "debuggatha.reopenFinding": "reopened",
  };

  /** Evaluates the `a == b && (c == d || e == f)` subset of VS Code `when` clauses the menu uses. */
  const matches = (when: string, viewItem: string) =>
    new Function("ctx", `return (${when.replace(/(\w+) == ([\w-]+)/g, '(ctx.$1 === "$2")')});`)({
      view: "debuggatha-findings",
      viewItem,
    }) as boolean;

  it("offers an action on a finding exactly when the lifecycle allows that transition", () => {
    const items = manifest.contributes.menus["view/item/context"];
    expect(items.map((item) => item.command).sort()).toEqual(Object.keys(TARGET).sort());

    for (const status of STATUSES) {
      const offered = items
        .filter((item) => matches(item.when, `finding-${status}`))
        .map((item) => item.command)
        .sort();
      const legal = Object.entries(TARGET)
        .filter(([, target]) => canTransitionFinding(status, target))
        .map(([command]) => command)
        .sort();
      expect(offered, `menu for a ${status} finding`).toEqual(legal);
    }
  });

  it("offers nothing on file and project nodes", () => {
    for (const item of manifest.contributes.menus["view/item/context"]) {
      expect(matches(item.when, "file")).toBe(false);
      expect(matches(item.when, "project")).toBe(false);
    }
  });
});
