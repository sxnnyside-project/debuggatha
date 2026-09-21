import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";

const rootDir = z.string().optional().describe("Absolute path of the repository.");

const userMessage = (text: string) => ({
  messages: [{ role: "user" as const, content: { type: "text" as const, text } }],
});

const target = (root: string | undefined) => (root ? ` in "${root}"` : "");

const REVIEW_STEP =
  "Call `review_changes`. It reviews what changed in the working tree, including new files and deletions.";
const EXPLAIN_STEP = "Call `explain_finding` with its `ledgerId`.";

/** The loop every prompt teaches, built in one place so the prompts cannot drift apart. */
const loop = (firstStep: string) =>
  [
    `1. ${firstStep}`,
    "2. Fix the code for each finding in `changes.introduced` (and `changes.reopened`). Use the recommendation: apply its `edit` when it has one (check the result when `safety` is `review`), otherwise follow its `example`. Call `explain_finding` with the finding's `ledgerId` if you need the reasoning.",
    "3. Call `review_changes` again. `changes.fixed` should now list what you fixed and `changes.introduced` should be empty. Repeat until it is, or until a finding needs a decision from the user.",
    "4. Only when the code is right and the rule is wrong for this case, accept the finding with `suppress_finding` and a real reason, or with the inline comment `explain_finding` gives you. Never accept a finding just to make a review quiet, and tell the user every finding you accepted and why.",
  ].join("\n");

export function registerPrompts(server: McpServer): void {
  server.registerPrompt(
    "review-flow",
    {
      title: "Review and fix what you changed",
      description:
        "After writing or editing code: review the changes, fix what the review finds, and verify the fixes.",
      argsSchema: { rootDir },
    },
    ({ rootDir: root }) =>
      userMessage(
        `Review the code you just changed${target(root)} with Debuggatha, and fix what it finds:\n\n${loop(REVIEW_STEP)}\n\nFinish by summarizing what was introduced, what you fixed, and what you accepted.`,
      ),
  );

  server.registerPrompt(
    "fix-findings",
    {
      title: "Fix the open findings",
      description: "Work through the findings the ledger still has open, most severe first.",
      argsSchema: {
        rootDir,
        severity: z
          .enum(["critical", "high", "medium", "low"])
          .optional()
          .describe("Only findings of this severity and above."),
      },
    },
    ({ rootDir: root, severity }) =>
      userMessage(
        `List the open findings${target(root)} with \`list_findings\` (status "open"${
          severity ? `, severity ${severity} and above` : ""
        }), then fix them starting with the most severe. For each one:\n\n${loop(EXPLAIN_STEP)}`,
      ),
  );

  server.registerPrompt(
    "triage-findings",
    {
      title: "Triage open findings",
      description:
        "List the open findings and decide, one by one, whether to resolve or dismiss them.",
      argsSchema: { rootDir },
    },
    ({ rootDir: root }) =>
      userMessage(
        `List the open findings${target(root)} with the \`list_findings\` tool (status "open"), then propose resolving or dismissing each with \`update_finding\`.`,
      ),
  );
}
