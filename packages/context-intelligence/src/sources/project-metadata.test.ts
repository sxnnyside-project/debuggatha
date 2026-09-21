import { afterEach, describe, expect, it } from "bun:test";
import { withTempRepo } from "@debuggatha/testing";
import { buildProjectMetadataItems } from "./project-metadata.js";

let cleanup: (() => void) | undefined;
afterEach(() => {
  cleanup?.();
  cleanup = undefined;
});

describe("buildProjectMetadataItems", () => {
  it("prefers a LICENSE file over package.json's license field", () => {
    const repo = withTempRepo({
      LICENSE: "MIT License\n",
      "package.json": JSON.stringify({ name: "demo", license: "Apache-2.0" }),
    });
    cleanup = repo.cleanup;

    const items = buildProjectMetadataItems(repo.root);
    const license = items.find((i) => i.key === "license");
    expect(license?.value).toBe("LICENSE");
    expect(license?.source).toBe("LICENSE");
  });

  it("falls back to package.json's license field when there is no LICENSE file", () => {
    const repo = withTempRepo({ "package.json": JSON.stringify({ name: "demo", license: "MIT" }) });
    cleanup = repo.cleanup;
    const items = buildProjectMetadataItems(repo.root);
    expect(items.find((i) => i.key === "license")?.value).toBe("MIT");
  });

  it("extracts version and private-distribution facts", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo", version: "1.2.3", private: true }),
    });
    cleanup = repo.cleanup;
    const items = buildProjectMetadataItems(repo.root);
    expect(items.find((i) => i.key === "version")?.value).toBe("1.2.3");
    expect(items.find((i) => i.key === "distribution")?.value).toMatch(/private/);
  });

  it("returns no items for a repo with no package.json and no license", () => {
    const repo = withTempRepo({ "README.md": "# demo\n" });
    cleanup = repo.cleanup;
    expect(buildProjectMetadataItems(repo.root)).toEqual([]);
  });
});
