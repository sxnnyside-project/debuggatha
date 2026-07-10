# Canonical Review Pack Specification (PACK_SPEC.md)

This document defines the canonical engineering standard for all Debuggatha Review Packs. A Review Pack is a declarative knowledge artifact. It contains Rules and Knowledge Entries that are strictly consumed by the Review Engine. 

**Review Packs NEVER execute code.** They are schema-driven catalogs (similar to a Terraform provider schema or an ESLint plugin manifest). The execution logic lives entirely within `@debuggatha/skills`.

---

## 1. Metadata and Identity
Every Review Pack MUST declare a strict identity.

- **`id`**: A unique string identifier. Must be namespaced. Examples: `debuggatha/react`, `debuggatha/owasp`.
- **`version`**: Semantic versioning string (e.g., `1.0.0`). The Capability Registry respects SemVer for dependency resolution.
- **`displayName`**: Human-readable name used in CLI/VS Code outputs.
- **`kind`**: Must be either:
  - `stack`: Language, runtime, or framework-specific rules (e.g., TypeScript, Express).
  - `concern`: Broad engineering principles (e.g., Security, Accessibility, Architecture).

## 2. Dependencies and Compatibility
Review Packs form a DAG. There are NO optional dependencies in v1.

- **`dependsOn`**: Array of strict dependencies.
  - `packId` & `versionRange`: Depends on another pack (e.g., the React pack depends on the TypeScript pack).
  - `skillId`: Depends on a specific Skill being registered (e.g., an Architecture pack depending on `debuggatha/architecture-analyzer`).

## 3. Rule Schema
Rules are atomic, declarative constraints.

- **`id`**: Unique identifier scoped to the pack (e.g., `rule-strict-null`).
- **`packId`**: Reference back to the parent pack's `id`.
- **`statement`**: A concise, direct, imperative instruction. (e.g., "Always use `tsx` extensions for files containing JSX"). Do NOT include the rationale here.
- **`category`**: Must map strictly to the domain: `security`, `performance`, `architecture`, `style`.
- **`defaultSeverity`**: Base severity level: `critical`, `high`, `medium`, `low`.
- **`appliesTo`**: Defines when the rule survives policy assembly.
  - `{ kind: "always" }`: Globally applicable (e.g., OWASP).
  - `{ kind: "requires-language", language: string }`: Bound to a language (e.g., `typescript`).
  - `{ kind: "requires-framework", framework: string }`: Bound to a framework (e.g., `react`).
  - `{ kind: "glob", pattern: string }`: Bound to file names/paths (e.g., `**/*.test.ts`).
- **`knowledgeRefs`**: Every Rule MUST cite at least one Knowledge Entry (`id` array).
- **`contradicts`**: Optional. If this rule explicitly opposes another rule, list the opposing rule IDs here to enable deterministic conflict resolution.

## 4. Knowledge Entries
The underlying justification for a rule. **"No source, no finding."**

- **`id`**: Unique identifier scoped to the pack.
- **`title`**: Summary of the engineering concept.
- **`body`**: Detailed rationale, implementation guidance, and context. Must explain the *why*, not just the *what*.
- **`externalRefs`**: Optional array of URLs pointing to official docs, RFCs, or established community guidelines.

## 5. Evidence Schema
When the Review Engine produces a `Finding` based on a Rule, the finding must provide:
- **`Code Evidence`**: The specific file path, line number, and offending snippet.
- **`Source Evidence`**: The `packId` and `ruleId` that triggered the finding.

## 6. Development and Maintenance Expectations
- **No Placeholders**: A rule without actionable criteria is invalid and must not be committed.
- **No Empty Packs**: A Review Pack must contain at least one Rule and one Knowledge Entry.
- **False Positives/Negatives**: Packs MUST document known heuristic limitations in their Knowledge Entries.
- **Testing**: Every pack must pass `@debuggatha/knowledge-system`'s `validateReviewPack` before registration.
