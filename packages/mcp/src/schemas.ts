import { z } from "zod";

export const SEVERITIES = ["informational", "low", "medium", "high", "critical"] as const;
export const STATUSES = ["open", "acknowledged", "resolved", "dismissed", "reopened"] as const;

const severity = z.enum(SEVERITIES);
const status = z.enum(STATUSES);

export const rootDir = z
  .string()
  .optional()
  .describe(
    "Absolute path of the repository. Defaults to DEBUGGATHA_REPOSITORY_ROOT, then the first root the client shares.",
  );

export const reviewInput = {
  rootDir,
  packIds: z
    .array(z.string())
    .optional()
    .describe(
      "Extra Review Pack ids on top of the packs detected for the repository's stack. Listed packs win rule conflicts.",
    ),
  policyId: z
    .string()
    .optional()
    .describe("Assert the review policy id a previous review reported; fails if it changed."),
  persist: z
    .boolean()
    .optional()
    .describe("Record findings in .debuggatha/ledger.json. Defaults to true; false is read-only."),
  includeBaselined: z
    .boolean()
    .optional()
    .describe(
      "Also report findings the repository's baseline accepted. Defaults to false: with a baseline, only new findings are reported.",
    ),
  semantic: z
    .boolean()
    .optional()
    .describe(
      "Also have a model read the changed code for defects analyzers do not see. Needs a model configured when the server started (DEBUGGATHA_SEMANTIC). What it raises is a labeled suspicion with low confidence, each quoting the line it is about; it never closes or hides a finding.",
    ),
  analyzers: z
    .boolean()
    .optional()
    .describe(
      "Set false to run only the built-in detectors. Which external analyzers run otherwise is decided when the server starts (DEBUGGATHA_ANALYZERS); a call cannot enable more.",
    ),
};

const semanticRun = z.object({
  provider: z.string(),
  model: z.string().optional(),
  verified: z.number(),
  confirmed: z.number(),
  doubtful: z.number(),
  unsure: z.number(),
  detected: z.number(),
  rejected: z.number(),
  failed: z.number(),
  notes: z.array(z.string()),
  durationMs: z.number(),
});

const analyzerRun = z.object({
  id: z.string(),
  name: z.string(),
  license: z.string(),
  trust: z.enum(["safe", "runs-project-code", "network"]),
  status: z.enum(["ran", "skipped", "failed"]),
  reason: z.string().optional(),
  version: z.string().optional(),
  findings: z.number(),
  durationMs: z.number(),
});

export const findingsFilterInput = {
  rootDir,
  status: z.array(status).optional().describe("Only entries in these lifecycle states."),
  severity: z.array(severity).optional().describe("Only entries of these severities."),
  category: z.array(z.string()).optional().describe("Only entries in these categories."),
  file: z
    .string()
    .optional()
    .describe("Only entries in this file, as a repository-relative or absolute path."),
};

const finding = z
  .object({
    id: z.string(),
    title: z.string(),
    explanation: z.string(),
    severity,
    confidence: z.string(),
    category: z.string(),
    locations: z.array(z.object({ file: z.string() }).passthrough()),
    evidence: z.array(z.record(z.unknown())),
  })
  .passthrough();

const ledgerEntry = z
  .object({
    id: z.string(),
    fingerprint: z.record(z.unknown()),
    status,
    latestFinding: finding,
    history: z.array(z.record(z.unknown())),
  })
  .passthrough();

const idList = z.array(z.string());

/** A ledger entry described enough to talk about without fetching it. */
const findingRef = z.object({
  ledgerId: z.string().optional(),
  title: z.string(),
  file: z.string(),
  line: z.number().optional(),
  ruleId: z.string().optional(),
  severity,
});

export const reviewOutput = {
  summary: z
    .object({
      totalFindings: z.number(),
      findingsBySeverity: z.record(z.number()),
      appliedPacks: z.array(z.object({ id: z.string(), version: z.string() })),
    })
    .passthrough(),
  findings: z.array(finding),
  changes: z.object({
    introduced: z.array(findingRef),
    fixed: z.array(findingRef),
    reopened: z.array(findingRef),
  }),
  syncReport: z
    .object({
      newEntryIds: idList,
      unchangedEntryIds: idList,
      changedEntryIds: idList,
      autoResolvedEntryIds: idList,
      reopenedEntryIds: idList,
    })
    .passthrough(),
  suppressedFindings: z.array(z.record(z.unknown())),
  suppressedInline: z.array(
    z.object({
      file: z.string(),
      line: z.number().optional(),
      ruleId: z.string(),
      reason: z.string().optional(),
    }),
  ),
  baselined: z.number(),
  analyzers: z
    .array(analyzerRun)
    .describe("Every external analyzer considered: which ran, which did not, and why."),
  semantic: semanticRun
    .nullable()
    .describe("What the semantic pass did, or null when it was not asked for."),
  persisted: z.boolean(),
};

export const analyzersOutput = {
  analyzers: z.array(
    z.object({
      id: z.string(),
      name: z.string(),
      license: z.string(),
      homepage: z.string(),
      covers: z.string(),
      trust: z.enum(["safe", "runs-project-code", "network"]),
      applies: z.boolean(),
      installed: z.boolean(),
      version: z.string().optional(),
      willRun: z.boolean(),
      reason: z.string().optional(),
    }),
  ),
};

export const reviewChangesOutput = {
  ...reviewOutput,
  base: z.string().nullable(),
  changedFiles: z.array(z.string()),
  deletedFiles: z.array(z.string()),
  note: z.string().optional(),
};

export const explainOutput = {
  finding: z.object({
    ledgerId: z.string(),
    status,
    severity,
    confidence: z.string(),
    category: z.string(),
    title: z.string(),
    file: z.string(),
    line: z.number().optional(),
    column: z.number().optional(),
    excerpt: z.string().optional(),
  }),
  rule: z.object({ id: z.string().optional(), pack: z.string().optional() }),
  why: z.string(),
  fix: z.string(),
  example: z.object({ before: z.string(), after: z.string() }).optional(),
  edit: z
    .object({
      line: z.number(),
      startColumn: z.number(),
      endColumn: z.number(),
      replacement: z.string(),
      safety: z.enum(["safe", "review"]),
    })
    .optional(),
  ifYouAcceptIt: z.object({ inlineComment: z.string(), durable: z.string() }),
  history: z.array(
    z.object({ timestamp: z.string(), state: z.string(), comment: z.string().optional() }),
  ),
};

export const suppressOutput = {
  memoryItemId: z.string(),
  file: z.string(),
  ruleId: z.string(),
  reason: z.string(),
  effect: z.string(),
};

export const baselineOutput = {
  file: z.string(),
  findings: z.number(),
  files: z.number(),
  createdAt: z.string(),
  analyzers: z.array(analyzerRun),
};

export const listFindingsOutput = { total: z.number(), entries: z.array(ledgerEntry) };
export const entryOutput = { entry: ledgerEntry };
/** Tools whose payload is the domain's own object; only its shape as an object is promised. */
export const freeformOutput = z.object({}).passthrough();
