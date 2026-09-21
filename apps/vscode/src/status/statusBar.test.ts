import { beforeEach, describe, expect, it } from "vitest";
import { mock, resetMock } from "../test/vscode-mock.js";
import { StatusBar } from "./statusBar.js";

function create() {
  const subscriptions: unknown[] = [];
  const bar = new StatusBar({ subscriptions } as never);
  const item = mock.statusBarItems[0];
  if (!item) throw new Error("no status bar item");
  return { bar, item, subscriptions };
}

describe("status bar", () => {
  beforeEach(resetMock);

  it("appears ready as soon as the extension starts, and is owned by the extension", () => {
    const { item, subscriptions } = create();
    expect(item.shown).toBe(true);
    expect(item.text).toContain("Debuggatha");
    expect(item.tooltip).toBe("Debuggatha is ready.");
    expect(subscriptions).toHaveLength(1);
  });

  it("shows a spinner while reviewing, and a click cancels the review", () => {
    const { bar, item } = create();
    bar.reviewing("manual");
    expect(item.text).toContain("sync~spin");
    expect(item.command).toBe("debuggatha.cancelReview");
    expect(item.tooltip).toContain("Click to cancel");
  });

  it("says when it is the file you saved that is being reviewed", () => {
    const { bar, item } = create();
    bar.reviewing("save");
    expect(item.tooltip).toContain("the file you saved");
  });

  it("shows how many findings are open, and a click opens the findings", () => {
    const { bar, item } = create();
    bar.showFindings(3);
    expect(item.text).toBe("$(warning) Debuggatha: 3");
    expect(item.tooltip).toContain("3 open findings");
    expect(item.command).toBe("workbench.view.extension.debuggatha-explorer");

    bar.showFindings(1);
    expect(item.tooltip).toContain("1 open finding.");
  });

  it("looks clean when there is nothing open", () => {
    const { bar, item } = create();
    bar.showFindings(0);
    expect(item.text).toBe("$(check-all) Debuggatha");
    expect(item.tooltip).toContain("No open findings");
  });

  it("goes back to the count after a review is over or cancelled", () => {
    const { bar, item } = create();
    bar.showFindings(2);
    bar.reviewing("manual");
    bar.ready();
    expect(item.text).toBe("$(warning) Debuggatha: 2");
  });

  it("says a review failed, and a click shows the log", () => {
    const { bar, item } = create();
    bar.failed("not a repository");
    expect(item.text).toContain("$(error)");
    expect(item.tooltip).toContain("not a repository");
    expect(item.command).toBe("debuggatha.showLog");
  });
});
