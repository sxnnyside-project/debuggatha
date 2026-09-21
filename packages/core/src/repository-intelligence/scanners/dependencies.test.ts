import { afterEach, describe, expect, it } from "bun:test";
import { readdirSync } from "node:fs";
import { withTempRepo } from "../../testing/index.js";
import { scanDependencies } from "./dependencies.js";
import { scanStack } from "./stack.js";

function scanBoth(root: string) {
  const rootEntries = readdirSync(root);
  const stack = scanStack(root, rootEntries);
  const dependencies = scanDependencies(rootEntries, stack);
  return { stack, dependencies };
}

describe("scanDependencies", () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    cleanup?.();
    cleanup = undefined;
  });

  it("records lockfile presence without parsing its contents", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo" }),
      "package-lock.json": "{}",
    });
    cleanup = repo.cleanup;

    const { dependencies } = scanBoth(repo.root);

    expect(dependencies.profile.lockfiles).toContainEqual({
      file: "package-lock.json",
      detail: "package-lock.json present",
    });
  });

  it("surfaces declared versions only for recognized frameworks, not the whole dependency tree", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({
        name: "demo",
        dependencies: { react: "^18.2.0", "left-pad": "^1.0.0" },
      }),
    });
    cleanup = repo.cleanup;

    const { dependencies } = scanBoth(repo.root);

    expect(dependencies.profile.declaredVersions.react).toBe("^18.2.0");
    expect(dependencies.profile.declaredVersions["left-pad"]).toBeUndefined();
  });

  it("extracts Rust toolchain constraint from Cargo.toml", () => {
    const repo = withTempRepo({
      "Cargo.toml": '[package]\nname = "demo"\nrust-version = "1.75"\n',
    });
    cleanup = repo.cleanup;

    const { dependencies } = scanBoth(repo.root);

    expect(dependencies.profile.runtimeConstraints.rust).toBe("1.75");
  });

  it("extracts the declared tauri version from Cargo.toml dependencies", () => {
    const repo = withTempRepo({
      "Cargo.toml": '[package]\nname = "demo"\n\n[dependencies]\ntauri = "2.1"\n',
    });
    cleanup = repo.cleanup;

    const { dependencies } = scanBoth(repo.root);

    expect(dependencies.profile.declaredVersions.tauri).toBe("2.1");
  });

  it("records the node engine constraint and packageManager field", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({
        name: "demo",
        engines: { node: ">=20" },
        packageManager: "bun@1.2.20",
      }),
    });
    cleanup = repo.cleanup;

    const { dependencies } = scanBoth(repo.root);

    expect(dependencies.profile.runtimeConstraints.node).toBe(">=20");
    expect(dependencies.profile.runtimeConstraints.packageManager).toBe("bun@1.2.20");
  });
});
