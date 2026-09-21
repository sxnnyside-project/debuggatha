import { beforeEach, describe, expect, it } from "vitest";
import { mock, resetMock } from "../test/vscode-mock.js";
import { getSettings, onSettingsChanged } from "./settings.js";

describe("settings", () => {
  beforeEach(resetMock);

  it("adds no packs of its own by default, leaving stack detection to choose", () => {
    expect(getSettings().extraReviewPacks).toEqual([]);
  });

  it("reads the packs a user asks for", () => {
    mock.settings["debuggatha.extraReviewPacks"] = ["debuggatha/owasp"];
    expect(getSettings().extraReviewPacks).toEqual(["debuggatha/owasp"]);
  });

  it("calls back with fresh settings only when a debuggatha setting changes", () => {
    const seen: string[][] = [];
    onSettingsChanged((settings) => seen.push(settings.extraReviewPacks));

    mock.settings["debuggatha.extraReviewPacks"] = ["debuggatha/react"];
    for (const listener of mock.configListeners) {
      listener({ affectsConfiguration: (section) => section === "debuggatha" });
      listener({ affectsConfiguration: () => false });
    }
    expect(seen).toEqual([["debuggatha/react"]]);
  });
});
