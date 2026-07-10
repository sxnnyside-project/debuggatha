import { describe, expect, it } from "vitest";
import { fixtureKnowledgeEntry, fixturePack, fixtureRule } from "./internal/fixtures.js";
import { validateReviewPack } from "./validate.js";

describe("validateReviewPack", () => {
  it("accepts a well-formed pack", () => {
    expect(() => validateReviewPack(fixturePack())).not.toThrow();
  });

  it("accepts a well-formed pack with pack and skill dependencies", () => {
    const pack = fixturePack({
      dependsOn: [{ packId: "rust", versionRange: "^1.0.0" }, { skillId: "change-impact" }],
    });
    expect(() => validateReviewPack(pack)).not.toThrow();
  });

  it("rejects a pack with a blank id", () => {
    expect(() => validateReviewPack(fixturePack({ id: "  " }))).toThrow(
      /pack\.id must be a non-empty string/,
    );
  });

  it("rejects a pack whose version is not semver-shaped", () => {
    expect(() => validateReviewPack(fixturePack({ version: "latest" }))).toThrow(
      /not a semver-shaped string/,
    );
  });

  it("rejects duplicate rule ids within a pack", () => {
    const pack = fixturePack({
      rules: [fixtureRule({ id: "dup" }), fixtureRule({ id: "dup" })],
    });
    expect(() => validateReviewPack(pack)).toThrow(/duplicate rule id "dup"/);
  });

  it("rejects duplicate knowledge entry ids within a pack", () => {
    const pack = fixturePack({
      knowledge: [fixtureKnowledgeEntry({ id: "dup" }), fixtureKnowledgeEntry({ id: "dup" })],
    });
    expect(() => validateReviewPack(pack)).toThrow(/duplicate knowledge entry id "dup"/);
  });

  it("rejects a rule whose packId does not match its containing pack", () => {
    const pack = fixturePack({ id: "real-pack", rules: [fixtureRule({ packId: "wrong-pack" })] });
    expect(() => validateReviewPack(pack)).toThrow(
      /does not match its containing pack "real-pack"/,
    );
  });

  it("rejects a rule with no knowledgeRefs", () => {
    const pack = fixturePack({ rules: [fixtureRule({ knowledgeRefs: [] })] });
    expect(() => validateReviewPack(pack)).toThrow(/has no knowledgeRefs/);
  });

  it("rejects a rule citing a knowledgeRefs entry that doesn't exist in the pack", () => {
    const pack = fixturePack({ rules: [fixtureRule({ knowledgeRefs: ["does-not-exist"] })] });
    expect(() => validateReviewPack(pack)).toThrow(
      /cites unknown knowledgeRefs entry "does-not-exist"/,
    );
  });

  it("rejects a dependsOn entry declaring both packId and skillId", () => {
    const pack = fixturePack({
      dependsOn: [{ packId: "rust", versionRange: "^1.0.0", skillId: "x" } as never],
    });
    expect(() => validateReviewPack(pack)).toThrow(/declares both packId and skillId/);
  });

  it("rejects a dependsOn entry declaring neither packId nor skillId", () => {
    const pack = fixturePack({ dependsOn: [{} as never] });
    expect(() => validateReviewPack(pack)).toThrow(/declares neither a packId nor a skillId/);
  });

  it("rejects a pack dependency missing a versionRange", () => {
    const pack = fixturePack({ dependsOn: [{ packId: "rust", versionRange: "" }] });
    expect(() => validateReviewPack(pack)).toThrow(/missing a non-empty versionRange/);
  });

  it("aggregates every issue found instead of stopping at the first", () => {
    const pack = fixturePack({
      id: "",
      version: "not-semver",
      rules: [fixtureRule({ knowledgeRefs: [] })],
    });
    let thrown: unknown;
    try {
      validateReviewPack(pack);
    } catch (error) {
      thrown = error;
    }
    expect(thrown).toBeInstanceOf(Error);
    const message = (thrown as Error).message;
    expect(message).toMatch(/pack\.id must be a non-empty string/);
    expect(message).toMatch(/not a semver-shaped string/);
    expect(message).toMatch(/has no knowledgeRefs/);
  });
});
