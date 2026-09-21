import { afterEach, describe, expect, it } from "bun:test";
import { readdirSync } from "node:fs";
import { withTempRepo } from "@debuggatha/testing";
import { scanStack } from "./stack.js";

function scan(root: string) {
  return scanStack(root, readdirSync(root));
}

describe("scanStack", () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    cleanup?.();
    cleanup = undefined;
  });

  it("detects Rust with evidence pointing at Cargo.toml", () => {
    const repo = withTempRepo({ "Cargo.toml": '[package]\nname = "demo"\n' });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);

    const rust = profile.languages.find((s) => s.value === "Rust");
    expect(rust).toBeDefined();
    expect(rust?.evidence).toEqual([
      { file: "Cargo.toml", detail: "Cargo.toml present at repository root" },
    ]);
  });

  it("detects Tauri only when Cargo.toml content mentions it, not from the filename alone", () => {
    const withoutTauri = withTempRepo({ "Cargo.toml": '[package]\nname = "demo"\n' });
    const withTauri = withTempRepo({
      "Cargo.toml": '[package]\nname = "demo"\n\n[dependencies]\ntauri = "2.0"\n',
    });

    const plain = scan(withoutTauri.root);
    const tauri = scan(withTauri.root);
    withoutTauri.cleanup();
    withTauri.cleanup();

    expect(plain.profile.frameworks.find((s) => s.value === "Tauri")).toBeUndefined();
    expect(tauri.profile.frameworks.find((s) => s.value === "Tauri")).toBeDefined();
  });

  it("detects Flutter from pubspec.yaml content, distinct from plain Dart", () => {
    const dartOnly = withTempRepo({ "pubspec.yaml": "name: demo\n" });
    const flutter = withTempRepo({
      "pubspec.yaml": "name: demo\ndependencies:\n  flutter:\n    sdk: flutter\n",
    });

    const dartResult = scan(dartOnly.root);
    const flutterResult = scan(flutter.root);
    dartOnly.cleanup();
    flutter.cleanup();

    expect(dartResult.profile.languages.map((s) => s.value)).toContain("Dart");
    expect(dartResult.profile.frameworks.find((s) => s.value === "Flutter")).toBeUndefined();
    expect(flutterResult.profile.frameworks.find((s) => s.value === "Flutter")).toBeDefined();
    expect(flutterResult.profile.platformTargets).toContain("mobile");
  });

  it("merges multiple pieces of evidence for the same package manager", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo", packageManager: "bun@1.2.20" }),
      "bun.lock": "",
    });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);

    const bun = profile.packageManagers.find((s) => s.value === "Bun");
    expect(bun?.evidence).toHaveLength(2);
    expect(profile.runtimes.find((s) => s.value === "Bun")).toBeDefined();
  });

  it("detects a monorepo from package.json workspaces and cites the field as evidence", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo", workspaces: ["packages/*"] }),
    });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);

    expect(profile.workspaceType).toBe("monorepo");
    expect(profile.workspaceEvidence).toContainEqual({
      file: "package.json",
      detail: '"workspaces" field present',
    });
  });

  it("defaults to single-package when no monorepo marker is present", () => {
    const repo = withTempRepo({ "package.json": JSON.stringify({ name: "demo" }) });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);

    expect(profile.workspaceType).toBe("single-package");
    expect(profile.workspaceEvidence).toHaveLength(0);
  });

  it("detects web and desktop platform targets from React + Tauri together", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({
        name: "demo",
        dependencies: { react: "^18.0.0", "@tauri-apps/api": "^2.0.0" },
      }),
    });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);

    expect(profile.platformTargets).toEqual(["desktop", "web"]);
  });

  it("does not assert a platform target when no frontend/desktop/mobile framework is detected", () => {
    const repo = withTempRepo({ "go.mod": "module demo\n\ngo 1.22\n" });
    cleanup = repo.cleanup;

    const { profile } = scan(repo.root);

    expect(profile.platformTargets).toHaveLength(0);
  });

  it("exposes parsed package.json and raw Cargo.toml for Dependency Context to reuse", () => {
    const repo = withTempRepo({
      "package.json": JSON.stringify({ name: "demo", dependencies: { react: "^18.0.0" } }),
      "Cargo.toml": '[package]\nname = "demo"\n',
    });
    cleanup = repo.cleanup;

    const result = scan(repo.root);

    expect(result.packageJson?.dependencies?.react).toBe("^18.0.0");
    expect(result.cargoToml).toContain('name = "demo"');
  });
});
