import { describe, expect, it } from "vitest";
import { fixturePack } from "./internal/fixtures.js";
import { createInMemoryRegistry } from "./registry.js";
import type { SkillDescriptor } from "./types/skill.js";

const coreSkill: SkillDescriptor = {
  id: "stack-detection",
  tier: "core",
  description: "Detects the stack.",
};
const reviewSkill: SkillDescriptor = {
  id: "change-impact",
  tier: "analysis",
  description: "Structural change impact.",
};

describe("createInMemoryRegistry", () => {
  it("lists packs and skills, optionally filtered by kind/tier", () => {
    const stackPack = fixturePack({ id: "react-ts", kind: "stack" });
    const concernPack = fixturePack({ id: "owasp", kind: "concern" });
    const registry = createInMemoryRegistry([stackPack, concernPack], [coreSkill, reviewSkill]);

    expect(
      registry
        .listPacks()
        .map((p) => p.id)
        .sort(),
    ).toEqual(["owasp", "react-ts"]);
    expect(registry.listPacks("concern")).toEqual([concernPack]);
    expect(registry.listSkills("core")).toEqual([coreSkill]);
    expect(registry.listSkills()).toHaveLength(2);
  });

  it("resolves a pack by id, defaulting to the highest registered version", () => {
    const v1 = fixturePack({ id: "react-ts", version: "1.0.0" });
    const v2 = fixturePack({ id: "react-ts", version: "2.1.0" });
    const registry = createInMemoryRegistry([v1, v2], []);

    expect(registry.getPack("react-ts")?.version).toBe("2.1.0");
  });

  it("resolves a pack by id against a caret version range", () => {
    const v1 = fixturePack({ id: "react-ts", version: "1.2.0" });
    const v2 = fixturePack({ id: "react-ts", version: "2.0.0" });
    const registry = createInMemoryRegistry([v1, v2], []);

    expect(registry.getPack("react-ts", "^1.0.0")?.version).toBe("1.2.0");
  });

  it("returns undefined for an unregistered pack id", () => {
    const registry = createInMemoryRegistry([], []);
    expect(registry.getPack("does-not-exist")).toBeUndefined();
  });

  it("resolves a pack's transitive pack dependency", () => {
    const rust = fixturePack({ id: "rust", version: "1.0.0", dependsOn: [] });
    const tauri = fixturePack({
      id: "tauri",
      version: "1.0.0",
      dependsOn: [{ packId: "rust", versionRange: "^1.0.0" }],
    });
    const registry = createInMemoryRegistry([rust, tauri], []);

    const { resolved, missing } = registry.resolveDependencies(["tauri"]);
    expect(missing).toEqual([]);
    expect(resolved.map((p) => p.id).sort()).toEqual(["rust", "tauri"]);
  });

  it("reports a missing pack dependency instead of silently dropping it", () => {
    const tauri = fixturePack({
      id: "tauri",
      version: "1.0.0",
      dependsOn: [{ packId: "rust", versionRange: "^1.0.0" }],
    });
    const registry = createInMemoryRegistry([tauri], []);

    const { resolved, missing } = registry.resolveDependencies(["tauri"]);
    expect(resolved.map((p) => p.id)).toEqual(["tauri"]);
    expect(missing).toEqual([{ packId: "rust", versionRange: "^1.0.0" }]);
  });

  it("reports a missing skill dependency", () => {
    const tauri = fixturePack({
      id: "tauri",
      version: "1.0.0",
      dependsOn: [{ skillId: "change-impact" }],
    });
    const registryWithoutSkill = createInMemoryRegistry([tauri], []);
    const registryWithSkill = createInMemoryRegistry([tauri], [reviewSkill]);

    expect(registryWithoutSkill.resolveDependencies(["tauri"]).missing).toEqual([
      { skillId: "change-impact" },
    ]);
    expect(registryWithSkill.resolveDependencies(["tauri"]).missing).toEqual([]);
  });

  it("reports an entirely unregistered top-level requested pack as missing", () => {
    const registry = createInMemoryRegistry([], []);
    const { resolved, missing } = registry.resolveDependencies(["ghost-pack"]);
    expect(resolved).toEqual([]);
    expect(missing).toEqual([{ packId: "ghost-pack", versionRange: "*" }]);
  });
});
