import { readFileSync, writeFileSync } from "node:fs";
import { loadLedger } from "@debuggatha/engine";
import { afterAll, beforeEach, describe, expect, it } from "vitest";
import { suppressInline } from "../commands/suppress.js";
import { ruleOf } from "../findings/present.js";
import { makeRepo, removeRepos, seedLedger } from "../test/fixtures.js";
import { harness } from "../test/harness.js";
import {
  type CodeAction,
  documentFor,
  mock,
  Position,
  Range,
  resetMock,
} from "../test/vscode-mock.js";
import { FindingCodeActionProvider } from "./codeActions.js";
import { FindingHoverProvider } from "./hover.js";

afterAll(removeRepos);

const SOURCE = 'var x: any = 1;\nif (x == 2) { eval("1"); }\n';

function setup() {
  const { root, badFile } = makeRepo(SOURCE);
  seedLedger(root);
  const { services } = harness(root);
  services.refreshFromLedger();
  const diagnostics = mock.collections.get("debuggatha")?.entries.get(badFile) ?? [];
  const document = () => documentFor(badFile);
  return { root, badFile, services, diagnostics, document };
}

const actionsFor = (
  provider: FindingCodeActionProvider,
  document: ReturnType<typeof documentFor>,
  diagnostics: unknown[],
  line: number,
) =>
  provider.provideCodeActions(
    document as never,
    new Range(line, 0, line, 1) as never,
    {
      diagnostics: diagnostics.filter((d) => (d as { range: Range }).range.start.line === line),
    } as never,
  ) as unknown as CodeAction[];

describe("hover provider", () => {
  beforeEach(resetMock);

  it("shows the finding under the cursor, only where it is", () => {
    const { services, document } = setup();
    const provider = new FindingHoverProvider(services.index);
    // `var` is columns 0-3 on line 0.
    const on = provider.provideHover(document() as never, new Position(0, 1) as never);
    expect(on).toBeDefined();
    const text = JSON.stringify(on?.contents);
    expect(text).toContain("no-var");

    // Far right of the line, past everything any finding covers: nothing.
    expect(
      provider.provideHover(document() as never, new Position(0, 14) as never),
    ).toBeUndefined();
    // A line with no finding.
    expect(provider.provideHover(document() as never, new Position(5, 0) as never)).toBeUndefined();
  });

  it("may run only Debuggatha's own commands from its links", () => {
    const { services, document } = setup();
    const hover = new FindingHoverProvider(services.index).provideHover(
      document() as never,
      new Position(0, 1) as never,
    );
    const contents = (hover?.contents ?? []) as unknown as {
      isTrusted: { enabledCommands: string[] };
    }[];
    const trusted = contents[0]?.isTrusted;
    const commands = trusted?.enabledCommands ?? [];
    expect(commands.length).toBeGreaterThan(0);
    expect(commands.every((command) => command.startsWith("debuggatha."))).toBe(true);
  });
});

describe("code action provider", () => {
  beforeEach(resetMock);

  it("offers the exact fix on a finding that has one, preferred only when it is safe", () => {
    const { services, diagnostics, document } = setup();
    const provider = new FindingCodeActionProvider(services.index);
    const actions = actionsFor(provider, document(), diagnostics, 0);

    // Two findings on this line have an edit: `var` (safe) and `any` (needs a look).
    const edits = actions.filter((action) => action.edit);
    expect(edits).toHaveLength(2);
    const safe = edits.find((action) => action.edit?.replacements[0]?.text === "let");
    expect(safe?.title).toContain("Debuggatha:");
    expect(safe?.isPreferred).toBe(true);
    expect(safe?.edit?.replacements[0]?.range.start.character).toBe(0);

    const review = edits.find((action) => action.edit?.replacements[0]?.text === "unknown");
    expect(review?.isPreferred).toBe(false); // never applied by an "apply all preferred fixes"
  });

  it("does not offer an edit for a finding that has none, but still offers the rest", () => {
    const { services, diagnostics, document } = setup();
    const onlyEval = diagnostics.filter((d) => d.code === "no-eval");
    const actions = actionsFor(
      new FindingCodeActionProvider(services.index),
      document(),
      onlyEval,
      1,
    );
    expect(actions.some((action) => action.edit)).toBe(false);
    const titles = actions.map((action) => action.title);
    expect(titles.some((title) => title.includes("accept no-eval"))).toBe(true);
    expect(titles).toContain("Debuggatha: mark resolved");
    expect(titles).toContain("Debuggatha: dismiss");
    expect(titles).toContain("Debuggatha: open detail");
  });

  it("stops offering the edit once the line is not what the review read", () => {
    const { services, diagnostics, badFile, document } = setup();
    writeFileSync(badFile, `var x: any = 2; // edited\n${SOURCE.split("\n")[1]}\n`);
    const actions = actionsFor(
      new FindingCodeActionProvider(services.index),
      document(),
      diagnostics,
      0,
    );
    expect(actions.some((action) => action.edit)).toBe(false);
  });

  it("acts on the finding by its id, so the command survives a refresh", () => {
    const { services, diagnostics, document } = setup();
    const actions = actionsFor(
      new FindingCodeActionProvider(services.index),
      document(),
      diagnostics,
      0,
    );
    const resolve = actions.find((action) => action.title === "Debuggatha: mark resolved");
    const id = resolve?.command?.arguments?.[0] as string;
    expect(services.index.entry(id)).toBeDefined();
    expect(resolve?.command?.command).toBe("debuggatha.resolveFindingById");
  });

  it("ignores diagnostics that are not Debuggatha's", () => {
    const { services, document } = setup();
    const foreign = { source: "eslint", code: "no-var", range: new Range(0, 0, 0, 3) };
    expect(
      actionsFor(new FindingCodeActionProvider(services.index), document(), [foreign], 0),
    ).toEqual([]);
  });
});

describe("accepting a finding in code", () => {
  beforeEach(resetMock);

  const eval_ = (services: ReturnType<typeof setup>["services"]) => {
    const entry = [...loadLedgerEntries(services)].find(
      (e) => ruleOf(e.latestFinding) === "no-eval",
    );
    if (!entry) throw new Error("no eval finding");
    return entry.id;
  };
  const loadLedgerEntries = (services: ReturnType<typeof setup>["services"]) =>
    loadLedger(services.rootDir as string).entries;

  it("inserts the reason above the line, indented like it, and asks for none when one is given", async () => {
    const { services } = setup();
    const applied = await suppressInline(services, eval_(services), "  legacy path  ");
    expect(applied).toBe(true);
    const insertion = mock.applied[0]?.insertions[0];
    expect(insertion?.position.line).toBe(1);
    expect(insertion?.text).toBe("// debuggatha-ignore-next-line no-eval -- legacy path\n");
  });

  it("asks why when no reason is given, and does nothing if the person declines", async () => {
    const { services } = setup();
    mock.inputAnswer = undefined;
    expect(await suppressInline(services, eval_(services))).toBe(false);
    expect(mock.applied).toEqual([]);

    mock.inputAnswer = "reviewed";
    expect(await suppressInline(services, eval_(services))).toBe(true);
    expect(mock.applied[0]?.insertions[0]?.text).toContain("-- reviewed");
  });

  it("refuses an empty reason", async () => {
    const { services } = setup();
    expect(await suppressInline(services, eval_(services), "   ")).toBe(false);
    expect(mock.applied).toEqual([]);
  });

  it("says a comment cannot silence a finding that comes from an external tool", async () => {
    const { root, services } = setup();
    // Turn one entry into an analyzer's finding in the ledger, as a review with Ruff would.
    const path = `${root}/.debuggatha/ledger.json`;
    const raw = JSON.parse(readFileSync(path, "utf8"));
    const entry = raw.entries[0];
    entry.latestFinding.evidence = [
      { kind: "external-analyzer", tool: "ruff", ruleId: "F401", license: "MIT" },
    ];
    writeFileSync(path, JSON.stringify(raw));
    services.refreshFromLedger();

    expect(await suppressInline(services, entry.id, "why")).toBe(false);
    expect(mock.infos.join(" ")).toContain("external tool");
    expect(mock.applied).toEqual([]);
  });

  it("reports a finding that is gone", async () => {
    const { services } = setup();
    expect(await suppressInline(services, "no-such-id", "why")).toBe(false);
    expect(mock.errors).toEqual(["That finding is no longer in the ledger."]);
  });
});
