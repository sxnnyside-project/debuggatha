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
    },
    {
      id: "know-release-lockfiles",
      title: "Build Determinism",
      body: "Without a lockfile, a minor patch to a transitive dependency can break the production build, even if the source code did not change. Lockfiles guarantee that what is tested locally is exactly what is deployed.",
      externalRefs: ["https://docs.npmjs.com/cli/v10/configuring-npm/package-lock-json"],
    },
    {
      id: "know-release-semver",
      title: "Semantic Versioning Contract",
      body: "SemVer provides a contract for consumers. A breaking change in a minor or patch release destroys consumer trust and breaks dependent systems.",
      externalRefs: ["https://semver.org/"],
    },
    {
      id: "know-release-immutable",
      title: "Immutable Deployments",
      body: "Rebuilding source code per environment introduces the risk that dependencies or toolchains have drifted between the staging build and the production build. Build once, deploy everywhere.",
      externalRefs: ["https://12factor.net/build-release-run"],
    },
  ],
};
