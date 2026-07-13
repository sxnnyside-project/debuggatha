import type { Capability, RepositoryContext } from "@debuggatha/repository-intelligence";
import type {
  Category,
  ReviewPackReference,
  ReviewRequest,
  Severity,
} from "@debuggatha/review-engine";
import { deepFreeze } from "./internal/deep-freeze.js";
import { canonicalStringify, KNOWLEDGE_SCHEMA_VERSION, sha256Hex } from "./internal/hash.js";
import type { CapabilityRegistry } from "./registry.js";
import type { PackDependency } from "./types/pack.js";
import type { ConflictRecord, PolicyRule, PolicyRuleOrigin, ReviewPolicy } from "./types/policy.js";
import type { RuleScope } from "./types/rule.js";

/**
 * Pure: `(RepositoryContext, ReviewRequest, CapabilityRegistry) →
 * ReviewPolicy` (Architecture doc §4 "Rule Resolution"). Throws — fail
 * closed, never silently degrades — when a requested pack (or a
 * transitive dependency of one) can't be resolved against the registry
 * (Architecture doc §9).
 */
export function resolvePolicy(
  context: RepositoryContext,
  request: ReviewRequest,
  registry: CapabilityRegistry,
): ReviewPolicy {
  const requestedPackIds = request.requestedPackIds;

  const { resolved, missing } = registry.resolveDependencies(requestedPackIds);
  if (missing.length > 0) {
    throw new Error(
      `Cannot resolve review policy: ${missing.length} unresolved dependenc${
        missing.length === 1 ? "y" : "ies"
      }: ${missing.map(describeDependency).join(", ")}`,
    );
  }

  const packRequestIndex = (packId: string): number => {
    const index = requestedPackIds.indexOf(packId);
    return index === -1 ? Number.POSITIVE_INFINITY : index;
  };

  const packCandidates: Candidate[] = [];
  for (const pack of resolved) {
    for (const rule of pack.rules) {
      if (!ruleApplies(rule.appliesTo, context.capabilities)) continue;
      packCandidates.push({
        id: rule.id,
        statement: rule.statement,
        category: rule.category,
        defaultSeverity: rule.defaultSeverity,
        origin: { kind: "pack", packId: pack.id, packVersion: pack.version },
        contradicts: rule.contradicts ?? [],
        packRequestIndex: packRequestIndex(pack.id),
      });
    }
  }

  const criteriaCandidates: Candidate[] = context.criteria.rules.map((criteriaRule) => ({
    id: criteriaRule.id,
    statement: criteriaRule.description,
    category: undefined,
    defaultSeverity: undefined,
    origin: { kind: "criteria" },
    contradicts: [],
    packRequestIndex: Number.POSITIVE_INFINITY,
  }));

  const { survivors, conflicts } = resolveConflicts([...packCandidates, ...criteriaCandidates]);

  const rules: PolicyRule[] = survivors
    .map(
      (candidate): PolicyRule => ({
        id: candidate.id,
        statement: candidate.statement,
        category: candidate.category,
        defaultSeverity: candidate.defaultSeverity,
        origin: candidate.origin,
      }),
    )
    .sort((a, b) => a.id.localeCompare(b.id));

  const contributingPackIds = new Set<string>();
  for (const rule of rules) {
    if (rule.origin.kind === "pack") contributingPackIds.add(rule.origin.packId);
  }

  const packRefs: ReviewPackReference[] = resolved
    .filter((pack) => contributingPackIds.has(pack.id))
    .map((pack) => ({ id: pack.id, version: pack.version }))
    .sort((a, b) => a.id.localeCompare(b.id));

  const id = computePolicyId(packRefs, context);

  if (request.requestedPolicyId && request.requestedPolicyId !== id) {
    throw new Error(
      `Requested policy id "${request.requestedPolicyId}" does not match the id "${id}" recomputed ` +
        "from the current repository state and requested pack ids — the repository's criteria or a " +
        "pack's registered version may have changed since this policy was originally resolved.",
    );
  }

  return deepFreeze({
    id,
    rules,
    packRefs,
    conflicts: [...conflicts].sort((a, b) => a.losingRuleId.localeCompare(b.losingRuleId)),
  });
}

// --- Rule scope matching -----------------------------------------------

/** Same normalization `@debuggatha/repository-intelligence`'s capability derivation applies to a `Capability.id` — so a Rule's authored `"React"`/`"Node.js"` string matches the capability id `"react"`/`"nodejs"` without either side needing to agree on casing or punctuation up front. */
function normalizeCapabilityId(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]/g, "");
}

/**
 * `glob` is deliberately not filtered here — Rule Resolution only has
 * `RepositoryContext.capabilities`, not a concrete file list, to check
 * against. A glob-scoped rule always survives Rule Resolution; matching
 * it against files actually in a review's scope is left to whatever
 * Skill executes per file (see package README "Technical decisions").
 *
 * `requires-framework`/`requires-language` both resolve against the same
 * flat `Capability[]` set (Epic 12.5) rather than
 * `StackProfile.frameworks`/`StackProfile.languages` — capabilities are
 * additive and not partitioned by `RuleScope`'s two kinds (some packs,
 * e.g. the Foundation Bundle's `bun`/`nodejs` rules, declare a runtime as
 * `requires-language` on purpose; that's a pack-authoring choice this
 * resolver doesn't second-guess, it just matches by id regardless of a
 * capability's own `kind`).
 */
function ruleApplies(scope: RuleScope, capabilities: readonly Capability[]): boolean {
  switch (scope.kind) {
    case "always":
      return true;
    case "requires-framework":
      return capabilities.some((cap) => cap.id === normalizeCapabilityId(scope.framework));
    case "requires-language":
      return capabilities.some((cap) => cap.id === normalizeCapabilityId(scope.language));
    case "glob":
      return true;
  }
}

// --- Conflict detection and precedence (ADR-0003) -----------------------

interface Candidate {
  id: string;
  statement: string;
  category: Category | undefined;
  defaultSeverity: Severity | undefined;
  origin: PolicyRuleOrigin;
  contradicts: string[];
  packRequestIndex: number;
}

function resolveConflicts(candidates: Candidate[]): {
  survivors: Candidate[];
  conflicts: ConflictRecord[];
} {
  const loserIndices = new Set<number>();
  const conflicts: ConflictRecord[] = [];
  const seenPairs = new Set<string>();

  const recordConflict = (aIdx: number, bIdx: number, reason: string): void => {
    const pairKey = aIdx < bIdx ? `${aIdx}:${bIdx}` : `${bIdx}:${aIdx}`;
    if (seenPairs.has(pairKey)) return;
    seenPairs.add(pairKey);

    const a = candidates[aIdx];
    const b = candidates[bIdx];
    if (!a || !b) return;
    const decision = decidePrecedence(a, b, aIdx, bIdx);
    loserIndices.add(decision.loserIdx);
    conflicts.push({ winningRuleId: decision.winner.id, losingRuleId: decision.loser.id, reason });
  };

  // Trigger 1 (explicit, ADR-0003 #1): a pack rule names another
  // candidate's id in its `contradicts` list.
  for (let i = 0; i < candidates.length; i++) {
    const candidate = candidates[i];
    if (!candidate) continue;
    for (const contradictedId of candidate.contradicts) {
      for (let j = 0; j < candidates.length; j++) {
        if (i === j) continue;
        if (candidates[j]?.id === contradictedId) {
          recordConflict(
            i,
            j,
            `rule "${candidate.id}" explicitly declares a contradiction with "${contradictedId}"`,
          );
        }
      }
    }
  }

  // Trigger 2 (automatic, narrow): a Pack rule's id exactly matches a
  // repository CriteriaRule's id. This package's operationalization of
  // ADR-0003's "(category, appliesTo) concern key" clause for the
  // Pack-vs-Criteria case — see package README "Technical decisions" for
  // why: `CriteriaRule` (repository-intelligence, frozen) carries neither
  // `category` nor `appliesTo`, so a literal concernKey match can't be
  // computed without inventing data Epic 1 doesn't provide.
  for (let i = 0; i < candidates.length; i++) {
    const candidate = candidates[i];
    if (candidate?.origin.kind !== "pack") continue;
    for (let j = 0; j < candidates.length; j++) {
      const other = candidates[j];
      if (i === j || !other) continue;
      if (other.origin.kind === "criteria" && other.id === candidate.id) {
        recordConflict(
          i,
          j,
          `pack rule "${candidate.id}" shares its id with a repository criteria rule of the same id`,
        );
      }
    }
  }

  const survivors = candidates.filter((_, index) => !loserIndices.has(index));
  return { survivors, conflicts };
}

function decidePrecedence(
  a: Candidate,
  b: Candidate,
  aIdx: number,
  bIdx: number,
): { winner: Candidate; loser: Candidate; winnerIdx: number; loserIdx: number } {
  // The repository's own CriteriaRule always wins over any Pack Rule (ADR-0003 #2).
  if (a.origin.kind === "criteria" && b.origin.kind === "pack") {
    return { winner: a, loser: b, winnerIdx: aIdx, loserIdx: bIdx };
  }
  if (b.origin.kind === "criteria" && a.origin.kind === "pack") {
    return { winner: b, loser: a, winnerIdx: bIdx, loserIdx: aIdx };
  }
  // Between two Pack rules, the pack named earlier in requestedPackIds wins.
  if (a.packRequestIndex !== b.packRequestIndex) {
    return a.packRequestIndex < b.packRequestIndex
      ? { winner: a, loser: b, winnerIdx: aIdx, loserIdx: bIdx }
      : { winner: b, loser: a, winnerIdx: bIdx, loserIdx: aIdx };
  }
  // Same pack (or both unrequested transitive dependencies) — deterministic tie-break.
  return a.id.localeCompare(b.id) <= 0
    ? { winner: a, loser: b, winnerIdx: aIdx, loserIdx: bIdx }
    : { winner: b, loser: a, winnerIdx: bIdx, loserIdx: aIdx };
}

// --- Helpers --------------------------------------------------------------

function describeDependency(dependency: PackDependency): string {
  if ("packId" in dependency) return `pack "${dependency.packId}" (${dependency.versionRange})`;
  return `skill "${dependency.skillId}"`;
}

/**
 * ADR-0005: a stable hash over the sorted, effective (post-dependency-
 * resolution, post-conflict-resolution) `ReviewPackReference` set, the
 * repository's `generatedAt` timestamp, a content fingerprint of
 * `RepositoryContext.criteria` only (not the whole `RepositoryContext` —
 * stack/dependency changes that don't affect which criteria rules exist
 * should not change the policy id), and `KNOWLEDGE_SCHEMA_VERSION`.
 */
function computePolicyId(packRefs: ReviewPackReference[], context: RepositoryContext): string {
  const payload = {
    schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
    packRefs: [...packRefs].sort(
      (a, b) => a.id.localeCompare(b.id) || a.version.localeCompare(b.version),
    ),
    repositoryGeneratedAt: context.generatedAt,
    criteriaFingerprint: sha256Hex(canonicalStringify(context.criteria)),
  };
  return sha256Hex(canonicalStringify(payload)).slice(0, 16);
}
