import { join } from "node:path";
import { loadLedger } from "@debuggatha/engine";
import { afterAll, afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { makeRepo, removeRepos } from "../test/fixtures.js";
import { harness } from "../test/harness.js";
import { mock, resetMock, Uri } from "../test/vscode-mock.js";
import { registerReviewOnSave, SAVE_DEBOUNCE_MS } from "./reviewOnSave.js";

afterAll(removeRepos);

function setup() {
  const { root, badFile } = makeRepo();
  const { services } = harness(root);
  registerReviewOnSave({ subscriptions: [] } as never, services);
  const save = (path: string, scheme = "file") => {
    for (const listener of mock.saveListeners) listener({ uri: new Uri(scheme, path) });
  };
  return { root, badFile, services, save };
}

const settle = () => vi.advanceTimersByTimeAsync(SAVE_DEBOUNCE_MS + 1_000);
const reviewed = (root: string) => loadLedger(root).entries.length > 0;

describe("review on save", () => {
  beforeEach(() => {
    resetMock();
    vi.useFakeTimers();
  });
  afterEach(() => vi.useRealTimers());

  it("does nothing unless the setting is on", async () => {
    const { root, badFile, save } = setup();
    save(badFile);
    await settle();
    expect(reviewed(root)).toBe(false);
  });

  it("reviews the saved file, quietly, once the editor has been idle a moment", async () => {
    const { root, badFile, save } = setup();
    mock.settings["debuggatha.reviewOnSave"] = true;

    save(badFile);
    await vi.advanceTimersByTimeAsync(SAVE_DEBOUNCE_MS - 50);
    expect(reviewed(root)).toBe(false); // still waiting: a burst of saves is one review
    await settle();

    expect(reviewed(root)).toBe(true);
    expect(mock.infos).toEqual([]);
    expect(mock.errors).toEqual([]);
    expect(mock.progressLocations).toEqual([10]); // the status bar, not a notification
  });

  it("a burst of saves is a single review", async () => {
    const { badFile, save } = setup();
    mock.settings["debuggatha.reviewOnSave"] = true;
    for (let i = 0; i < 5; i++) {
      save(badFile);
      await vi.advanceTimersByTimeAsync(100);
    }
    await settle();
    expect(mock.progressLocations).toHaveLength(1);
  });

  it("only ever reviews the file that was saved, whatever the depth", async () => {
    const { root, badFile, save } = setup();
    mock.settings["debuggatha.reviewOnSave"] = true;
    mock.settings["debuggatha.reviewDepth"] = "architectural";
    save(badFile);
    await settle();
    const files = new Set(loadLedger(root).entries.map((entry) => entry.fingerprint.file));
    expect(files).toEqual(new Set(["src/bad.ts"]));
  });

  it("leaves alone what is not the project's own code", async () => {
    const { root, save } = setup();
    mock.settings["debuggatha.reviewOnSave"] = true;
    save(join(root, "node_modules", "dep", "index.js"));
    save(join(root, ".debuggatha", "ledger.json"));
    save(join(root, "dist", "bundle.min.js"));
    save("/somewhere/else/outside.ts");
    save(join(root, "src", "bad.ts"), "untitled");
    await settle();
    expect(reviewed(root)).toBe(false);
    expect(mock.progressLocations).toEqual([]);
  });

  it("does nothing without a workspace", async () => {
    const { save } = setup();
    mock.workspaceFolders = undefined;
    mock.settings["debuggatha.reviewOnSave"] = true;
    save("/repo/a.ts");
    await settle();
    expect(mock.progressLocations).toEqual([]);
  });
});
