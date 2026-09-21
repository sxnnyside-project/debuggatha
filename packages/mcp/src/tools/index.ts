import { existsSync } from "node:fs";
import { join } from "node:path";
import {
  activateMemory,
  addMemoryItem,
  BASELINE_FILE,
  canTransitionFinding,
  confirmMemory,
  createBaseline,
  createMemoryItem,
  guidanceFor,
  inlineSuppressionComment,
  inventoryAnalyzers,
  isActiveFindingStatus,
  type LedgerEntryFilter,
  listChangedFiles,
  loadMemoryStore,
  saveMemoryStore,
  suggestMemory,
  toRepoRelative,
} from "@debuggatha/engine";
import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import type { ToolAnnotations } from "@modelcontextprotocol/sdk/types.js";
import { z } from "zod";
import {
  analyzersOutput,
  baselineOutput,
  entryOutput,
  explainOutput,
  findingsFilterInput,
  freeformOutput,
  listFindingsOutput,
  reviewChangesOutput,
  reviewInput,
  reviewOutput,
  rootDir,
  STATUSES,
  suppressOutput,
} from "../schemas.js";
import { reviewPayload } from "./review-payload.js";
import { analyzersFor, pipelineDeps, type RunReviewScope, runReview } from "./run-review.js";
import {
  structuredResult,
  type ToolResult,
  type ToolRuntimeContext,
  withToolLogging,
} from "./shared.js";

const READ_ONLY: ToolAnnotations = {
  readOnlyHint: true,
  destructiveHint: false,
  idempotentHint: true,
  openWorldHint: false,
};

// Reviews and status updates write `.debuggatha/ledger.json` inside the repository.
// Neither deletes anything (ledger history is append-only) and repeating a call converges.
const LEDGER_WRITE: ToolAnnotations = {
  readOnlyHint: false,
  destructiveHint: false,
  idempotentHint: true,
  openWorldHint: false,
};

interface ReviewArgs {
  rootDir?: string | undefined;
  packIds?: string[] | undefined;
  policyId?: string | undefined;
  persist?: boolean | undefined;
  includeBaselined?: boolean | undefined;
  analyzers?: boolean | undefined;
  semantic?: boolean | undefined;
}

export function registerTools(server: McpServer, ctx: ToolRuntimeContext): void {
  const review = (
    name: string,
    title: string,
    description: string,
    extraInput: z.ZodRawShape,
    toScope: (args: never) => RunReviewScope,
  ) => {
    server.registerTool(
      name,
      {
        title,
        description,
        inputSchema: { ...extraInput, ...reviewInput },
        outputSchema: reviewOutput,
        annotations: LEDGER_WRITE,
      },
      withToolLogging(ctx, name, async (args: ReviewArgs): Promise<ToolResult> => {
        const persist = args.persist !== false;
        const output = await runReview(ctx, {
          rootDir: await ctx.resolveRoot(args.rootDir),
          scope: toScope(args as never),
          packIds: args.packIds ?? [],
          policyId: args.policyId,
          persist,
          includeBaselined: args.includeBaselined === true,
          analyzers: args.analyzers,
          semantic: args.semantic,
        });
        return structuredResult(reviewPayload(output, persist));
      }),
    );
  };

  review(
    "review_diff",
    "Review a git diff",
    "Reviews the added lines of a unified git diff against the Review Packs that match the repository's stack. Every finding cites the rule that produced it.",
    {
      diff: z.string().describe("The raw unified git diff text."),
      base: z.string().optional().describe("The base commit or branch the diff was taken against."),
    },
    (args: { diff: string; base?: string }) => ({ kind: "diff", diff: args.diff, base: args.base }),
  );
  review(
    "review_files",
    "Review specific files",
    "Reviews whole files against the Review Packs that match the repository's stack.",
    { paths: z.array(z.string()).min(1).describe("Absolute paths of the files to review.") },
    (args: { paths: string[] }) => ({ kind: "files", paths: args.paths }),
  );
  review(
    "review_workspace",
    "Review the whole repository",
    "Reviews every source file in the repository, plus its architecture: dependency cycles, layer violations, and dead code.",
    {},
    () => ({ kind: "workspace" }),
  );

  server.registerTool(
    "review_changes",
    {
      title: "Review what changed",
      description:
        "Call this after you write or edit code. Reviews the files changed in the working tree since the last commit (or since `base`): edits, new files git does not track yet, and deletions. The response's `changes` says what your work introduced, what it fixed (findings that are gone because the code was corrected or the file removed), and what came back. Every finding has a `ledgerId` for explain_finding and suppress_finding, and a recommendation with the exact fix. Fix what you introduced, then call it again until `changes.introduced` is empty.",
      inputSchema: {
        ...reviewInput,
        base: z
          .string()
          .optional()
          .describe(
            "A git ref or range to measure changes against, e.g. origin/main...HEAD. Defaults to the last commit.",
          ),
      },
      outputSchema: reviewChangesOutput,
      annotations: LEDGER_WRITE,
    },
    withToolLogging(
      ctx,
      "review_changes",
      async (args: ReviewArgs & { base?: string | undefined }): Promise<ToolResult> => {
        const root = await ctx.resolveRoot(args.rootDir);
        const { changed, deleted: deletedInGit, base } = listChangedFiles(root, args.base);
        const persist = args.persist !== false;

        // A file the agent created and later removed never reached git, but the ledger remembers
        // what it held: an open finding in a file that no longer exists is fixed, by removal.
        const removedSinceLastReview = [
          ...new Set(
            ctx.deps
              .loadLedger(root)
              .entries.filter(
                (entry) =>
                  isActiveFindingStatus(entry.status) &&
                  !existsSync(join(root, entry.fingerprint.file)),
              )
              .map((entry) => entry.fingerprint.file),
          ),
        ];
        const deleted = [...new Set([...deletedInGit, ...removedSinceLastReview])].sort();
        const extra = { base, changedFiles: changed, deletedFiles: deleted };

        // The domain refuses a review of no files; nothing changed is an answer, not an error.
        if (changed.length === 0 && deleted.length === 0) {
          return structuredResult({
            summary: { totalFindings: 0, findingsBySeverity: {}, appliedPacks: [] },
            findings: [],
            changes: { introduced: [], fixed: [], reopened: [] },
            syncReport: {
              newEntryIds: [],
              unchangedEntryIds: [],
              changedEntryIds: [],
              autoResolvedEntryIds: [],
              reopenedEntryIds: [],
            },
            suppressedFindings: [],
            suppressedInline: [],
            baselined: 0,
            analyzers: [],
            semantic: null,
            persisted: persist,
            note: base
              ? `Nothing changed since ${base}.`
              : "Nothing has changed in this repository yet.",
            ...extra,
          });
        }

        const output = await runReview(ctx, {
          rootDir: root,
          // Deleted files go in too: reviewing a path that is gone is what resolves what it held.
          scope: { kind: "files", paths: [...changed, ...deleted] },
          packIds: args.packIds ?? [],
          policyId: args.policyId,
          persist,
          includeBaselined: args.includeBaselined === true,
          analyzers: args.analyzers,
          semantic: args.semantic,
        });
        return structuredResult(reviewPayload(output, persist, extra));
      },
    ),
  );

  server.registerTool(
    "explain_finding",
    {
      title: "Explain a finding",
      description:
        "Everything needed to act on one finding: where it is, why the pattern is a problem, how to fix it (an exact edit when the fix is mechanical, otherwise a before/after example), and what to do if the code is right and the rule is wrong.",
      inputSchema: { rootDir, findingId: z.string().describe("The finding's ledgerId.") },
      outputSchema: explainOutput,
      annotations: READ_ONLY,
    },
    withToolLogging(ctx, "explain_finding", async (args): Promise<ToolResult> => {
      const root = await ctx.resolveRoot(args.rootDir);
      const entry = ctx.deps.getEntry(ctx.deps.loadLedger(root), args.findingId);
      if (!entry) throw new Error(`No ledger entry with id "${args.findingId}".`);

      const finding = entry.latestFinding;
      const location = finding.locations[0];
      const ruleId = entry.fingerprint.ruleId;
      const guidance = ruleId ? guidanceFor(ruleId) : undefined;
      const recommendation = finding.recommendations[0];
      const excerpt = finding.evidence.find((e) => e.kind === "code");
      return structuredResult({
        finding: {
          ledgerId: entry.id,
          status: entry.status,
          severity: finding.severity,
          confidence: finding.confidence,
          category: finding.category,
          title: finding.title,
          file: entry.fingerprint.file,
          line: location?.lines?.start,
          column: location?.columns?.start,
          excerpt: excerpt && "excerpt" in excerpt ? excerpt.excerpt : undefined,
        },
        rule: { id: ruleId, pack: finding.originatingPack?.id },
        why: guidance?.why ?? finding.explanation,
        fix: recommendation?.summary ?? "No specific fix is recorded for this rule.",
        example: recommendation?.example,
        edit: recommendation?.edit,
        ifYouAcceptIt: {
          inlineComment: inlineSuppressionComment(
            entry.fingerprint.file,
            ruleId ?? "<rule>",
            "<why this is acceptable>",
          ),
          durable: `suppress_finding with this ledgerId and a reason accepts every ${ruleId ?? "such"} finding in ${entry.fingerprint.file}.`,
        },
        history: entry.history.map((event) => ({
          timestamp: event.timestamp,
          state: event.newState,
          comment: event.comment,
        })),
      });
    }),
  );

  server.registerTool(
    "suppress_finding",
    {
      title: "Accept a finding, with a reason",
      description:
        "Records that a finding is accepted: every finding of the same rule in the same file stops being reported, and the reason is kept in .debuggatha/memory.json for the team to read. Use it only when the code is right and the rule is wrong for this case, never to make a review quiet. A reason is required. Prefer fixing the code; to accept one line instead, add the inline comment explain_finding gives you.",
      inputSchema: {
        rootDir,
        findingId: z.string().describe("The finding's ledgerId."),
        reason: z
          .string()
          .min(8)
          .describe("Why this is acceptable: what makes the rule wrong here. Shown to reviewers."),
      },
      outputSchema: suppressOutput,
      // Hides findings from every later review until someone removes the memory item.
      annotations: { ...LEDGER_WRITE, destructiveHint: true },
    },
    withToolLogging(ctx, "suppress_finding", async (args): Promise<ToolResult> => {
      const root = await ctx.resolveRoot(args.rootDir);
      const ledger = ctx.deps.loadLedger(root);
      const entry = ctx.deps.getEntry(ledger, args.findingId);
      if (!entry) throw new Error(`No ledger entry with id "${args.findingId}".`);
      const ruleId = entry.fingerprint.ruleId;
      if (!ruleId) {
        throw new Error(
          "This finding has no rule behind it, so there is nothing to accept by rule.",
        );
      }

      const actor = "mcp-agent";
      const item = createMemoryItem({
        category: "suppression",
        fingerprintFile: entry.fingerprint.file,
        fingerprintRuleId: ruleId,
        fingerprintCategory: entry.fingerprint.category,
        confidence: "documented",
        rationale: args.reason,
        evidence: [
          { file: entry.fingerprint.file, detail: `Accepted: ${entry.latestFinding.title}` },
        ],
        origin: { kind: "user", actor },
      });
      let store = addMemoryItem(loadMemoryStore(root), item);
      store = suggestMemory(store, item.id, `mcp-${Date.now()}`);
      store = confirmMemory(store, item.id, actor, args.reason);
      store = activateMemory(store, item.id, actor);
      saveMemoryStore(store);

      // The ledger's picture follows: the finding is dismissed, with the same reason.
      if (canTransitionFinding(entry.status, "dismissed")) {
        ctx.deps.saveLedger(
          ctx.deps.updateFindingStatus(
            ledger,
            args.findingId,
            "dismissed",
            { kind: "manual", actor },
            args.reason,
          ),
        );
      }

      return structuredResult({
        memoryItemId: item.id,
        file: entry.fingerprint.file,
        ruleId,
        reason: args.reason,
        effect: `Reviews no longer report ${ruleId} in ${entry.fingerprint.file}. Remove the item from .debuggatha/memory.json to bring it back.`,
      });
    }),
  );

  server.registerTool(
    "create_baseline",
    {
      title: "Accept the current findings as a baseline",
      description:
        "Reviews the whole repository and records every finding it has today in .debuggatha/baseline.json. From then on, review tools report only findings that are new since the baseline (pass includeBaselined to see all). Use it to adopt Debuggatha on an existing repository without drowning in old findings.",
      inputSchema: { rootDir },
      outputSchema: baselineOutput,
      // Overwrites the previous baseline file, and hides findings from later reviews.
      annotations: { ...LEDGER_WRITE, destructiveHint: true },
    },
    withToolLogging(ctx, "create_baseline", async (args): Promise<ToolResult> => {
      const root = await ctx.resolveRoot(args.rootDir);
      const { baseline, count, analyzers } = createBaseline(
        {
          rootDir: root,
          sourceName: "debuggatha-mcp",
          // The same analyzers the reviews will run, or their findings would all look new.
          analyzers: analyzersFor(ctx, undefined),
          ...(ctx.cache ? { cache: ctx.cache } : {}),
        },
        pipelineDeps(ctx.deps),
      );
      return structuredResult({
        file: BASELINE_FILE,
        findings: count,
        files: new Set(baseline.entries.map((entry) => entry.file)).size,
        createdAt: baseline.createdAt,
        analyzers,
      });
    }),
  );

  server.registerTool(
    "list_analyzers",
    {
      title: "List external analyzers",
      description:
        "Lists the external analyzers (Ruff, Biome, gitleaks, ktlint, and others) that reviews can run: which are installed here, each one's license, and which a review would run. Debuggatha runs them as separate processes and reports their findings with the tool and its license; it does not include or install them. Tools that run project code or use the network run only when the server was started with them enabled (DEBUGGATHA_ANALYZERS).",
      inputSchema: { rootDir },
      outputSchema: analyzersOutput,
      annotations: READ_ONLY,
    },
    withToolLogging(ctx, "list_analyzers", async (args): Promise<ToolResult> => {
      const root = await ctx.resolveRoot(args.rootDir);
      return structuredResult({
        analyzers: inventoryAnalyzers(root, analyzersFor(ctx, undefined)),
      });
    }),
  );

  server.registerTool(
    "list_findings",
    {
      title: "List findings",
      description:
        "Lists findings recorded in the repository's ledger, optionally filtered by status, severity, category, or file.",
      inputSchema: findingsFilterInput,
      outputSchema: listFindingsOutput,
      annotations: READ_ONLY,
    },
    withToolLogging(ctx, "list_findings", async (args): Promise<ToolResult> => {
      const root = await ctx.resolveRoot(args.rootDir);
      const filter: LedgerEntryFilter = {};
      if (args.status !== undefined) filter.status = args.status;
      if (args.severity !== undefined) filter.severity = args.severity;
      if (args.category !== undefined) filter.category = args.category;
      if (args.file !== undefined) filter.file = toRepoRelative(root, args.file);
      const entries = ctx.deps.listEntries(ctx.deps.loadLedger(root), filter);
      return structuredResult({ total: entries.length, entries });
    }),
  );

  server.registerTool(
    "get_finding",
    {
      title: "Get a finding",
      description: "Returns one ledger entry, including its full lifecycle history.",
      inputSchema: { rootDir, findingId: z.string().describe("The ledger entry id.") },
      outputSchema: entryOutput,
      annotations: READ_ONLY,
    },
    withToolLogging(ctx, "get_finding", async (args): Promise<ToolResult> => {
      const root = await ctx.resolveRoot(args.rootDir);
      const entry = ctx.deps.getEntry(ctx.deps.loadLedger(root), args.findingId);
      if (!entry) throw new Error(`No ledger entry with id "${args.findingId}".`);
      return structuredResult({ entry });
    }),
  );

  server.registerTool(
    "update_finding",
    {
      title: "Update a finding's status",
      description:
        "Moves a ledger entry to a new lifecycle status and records the change in its history. Illegal transitions are rejected.",
      inputSchema: {
        rootDir,
        findingId: z.string().describe("The ledger entry id."),
        status: z.enum(STATUSES).describe("The status to move the finding to."),
        comment: z.string().optional().describe("Why the status changed; stored in the history."),
      },
      outputSchema: entryOutput,
      annotations: LEDGER_WRITE,
    },
    withToolLogging(ctx, "update_finding", async (args): Promise<ToolResult> => {
      const root = await ctx.resolveRoot(args.rootDir);
      const updated = ctx.deps.updateFindingStatus(
        ctx.deps.loadLedger(root),
        args.findingId,
        args.status,
        { kind: "manual", actor: "mcp-server" },
        args.comment,
      );
      ctx.deps.saveLedger(updated);
      return structuredResult({ entry: ctx.deps.getEntry(updated, args.findingId) });
    }),
  );

  server.registerTool(
    "repository_context",
    {
      title: "Repository context",
      description:
        "The full context Debuggatha resolves before reviewing: stack, dependencies, documentation, criteria, and understanding.",
      inputSchema: { rootDir },
      outputSchema: freeformOutput,
      annotations: READ_ONLY,
    },
    withToolLogging(ctx, "repository_context", async (args): Promise<ToolResult> => {
      const root = await ctx.resolveRoot(args.rootDir);
      return structuredResult({ ...ctx.deps.buildRepositoryContext(root, contextOptions(ctx)) });
    }),
  );

  server.registerTool(
    "repository_summary",
    {
      title: "Repository summary",
      description:
        "A compact view of the repository: detected stack and capabilities, dependencies, criteria, and ledger totals.",
      inputSchema: { rootDir },
      outputSchema: freeformOutput,
      annotations: READ_ONLY,
    },
    withToolLogging(ctx, "repository_summary", async (args): Promise<ToolResult> => {
      const root = await ctx.resolveRoot(args.rootDir);
      const context = ctx.deps.buildRepositoryContext(root, contextOptions(ctx));
      return structuredResult({
        rootDir: root,
        stack: context.stack,
        capabilities: context.capabilities,
        summary: ctx.deps.summarizeCapabilities(context.capabilities),
        dependencies: context.dependencies,
        criteria: context.criteria,
        ledgerSummary: ctx.deps.summarizeLedger(ctx.deps.loadLedger(root)),
      });
    }),
  );
}

export function contextOptions(ctx: Pick<ToolRuntimeContext, "cache">) {
  return ctx.cache ? { cache: ctx.cache } : {};
}
