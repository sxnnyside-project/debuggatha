import { describe, expect, it } from "bun:test";
import type { ModuleBoundary } from "../types.js";
import { buildLayerModel } from "./layer-model.js";

function boundary(id: string, name?: string): ModuleBoundary {
  return { id, dir: `/repo/${id}`, kind: "package", name, files: [] };
}

describe("buildLayerModel", () => {
  it("assigns known conventional layer names", () => {
    const model = buildLayerModel([
      boundary("packages/domain-users", "@x/domain-users"),
      boundary("packages/adapters-db", "@x/adapters-db"),
    ]);
    expect(model.assignments["packages/domain-users"]).toBe("domain");
    expect(model.assignments["packages/adapters-db"]).toBe("adapter");
  });

  it("assigns unknown for a boundary matching no vocabulary, instead of guessing", () => {
    const model = buildLayerModel([boundary("packages/whatever", "@x/whatever")]);
    expect(model.assignments["packages/whatever"]).toBe("unknown");
  });
});
