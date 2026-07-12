import type { ReviewPack } from "@debuggatha/knowledge-system";

export const releasePack: ReviewPack = {
  id: "debuggatha/release",
  version: "1.0.0",
  kind: "concern",
  displayName: "Release Engineering",
  dependsOn: [],
  rules: [
    {
      id: "no-hardcoded-secrets",
      packId: "debuggatha/release",
      statement:
        "Never hardcode API keys, passwords, or connection strings in source code. Inject them via environment variables or a secure secret manager.",
      category: "security",
      appliesTo: { kind: "always" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-release-secrets"],
      contradicts: undefined,
    },
    {
      id: "deterministic-builds",
      packId: "debuggatha/release",
      statement:
        "Use lockfiles (`package-lock.json`, `Cargo.lock`, `go.sum`) and commit them to version control to guarantee deterministic builds across environments.",
      category: "architecture",
      appliesTo: { kind: "always" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-release-lockfiles"],
      contradicts: undefined,
    },
    {
      id: "semantic-versioning",
      packId: "debuggatha/release",
      statement:
        "Adhere to Semantic Versioning (SemVer). Do not introduce breaking changes without bumping the major version number.",
      category: "architecture",
      appliesTo: { kind: "always" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-release-semver"],
      contradicts: undefined,
    },
    {
      id: "immutable-artifacts",
      packId: "debuggatha/release",
      statement:
        "Build artifacts (like Docker images or binaries) must be immutable. Deploy the exact same artifact to staging and production, modifying only the injected environment variables.",
      category: "architecture",
      appliesTo: { kind: "always" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-release-immutable"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-release-secrets",
      title: "Hardcoded Credentials",
      body: "Hardcoding secrets into source control means anyone with read access to the repository has access to production infrastructure. It also makes credential rotation incredibly dangerous and slow.",
      externalRefs: ["https://12factor.net/config"],
      limitations: [
        "A naive string/pattern scan cannot distinguish a real secret from a placeholder, example value, or test fixture (e.g. `API_KEY=your-key-here` in a README or `sk_test_...` Stripe test keys meant to be public) — flagging every string that looks key-shaped produces false positives in docs and test files.",
      ],
    },
    {
      id: "know-release-lockfiles",
      title: "Build Determinism",
      body: "Without a lockfile, a minor patch to a transitive dependency can break the production build, even if the source code did not change. Lockfiles guarantee that what is tested locally is exactly what is deployed.",
      externalRefs: ["https://docs.npmjs.com/cli/v10/configuring-npm/package-lock-json"],
      limitations: [
        "A missing lockfile at the repo root is not automatically a violation for a published library package, where consumers are expected to resolve their own dependency tree — this rule targets deployable applications/services, not npm packages intentionally distributed without a committed lockfile.",
      ],
    },
    {
      id: "know-release-semver",
      title: "Semantic Versioning Contract",
      body: "SemVer provides a contract for consumers. A breaking change in a minor or patch release destroys consumer trust and breaks dependent systems.",
      externalRefs: ["https://semver.org/"],
      limitations: [
        "Detecting an actual breaking change requires understanding the package's public API surface and how it's used by consumers — a diff-only scan can't tell whether an internal refactor changed an exported type's shape versus only touched implementation details never exposed, so this is hard to enforce mechanically without an API-diffing tool.",
      ],
    },
    {
      id: "know-release-immutable",
      title: "Immutable Deployments",
      body: "Rebuilding source code per environment introduces the risk that dependencies or toolchains have drifted between the staging build and the production build. Build once, deploy everywhere.",
      externalRefs: ["https://12factor.net/build-release-run"],
      limitations: [
        "Environment-specific compile-time constants that must legitimately differ per environment (e.g. a build-time feature flag baked in via a bundler define, or platform-specific native binaries for a multi-arch release) can look like a per-environment rebuild without being a violation of build-once — the rule targets rebuilding the same artifact from source per environment, not artifacts that are inherently environment-specific by design.",
      ],
    },
  ],
};
