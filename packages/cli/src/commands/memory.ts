import {
  activateMemory,
  archiveMemory,
  confirmMemory,
  deprecateMemory,
  loadMemoryStore,
  type MemoryItem,
  rejectMemory,
  removeMemoryItem,
  saveMemoryStore,
  suggestMemory,
} from "@debuggatha/engine";
import type { Command } from "commander";
import type { CliLogger } from "../utils/logger.js";

/**
 * Repository Memory adapter surface (inspect, confirm, reject, remove). Thin
 * wiring only — every action below delegates directly to
 * `core/repository-memory`'s already-tested API via
 * `@debuggatha/core`; this file adds no lifecycle logic of its own.
 */
function describeItem(item: MemoryItem): string {
  const detail =
    item.category === "convention" || item.category === "decision"
      ? "rule" in item
        ? item.rule
        : item.decision
      : item.category === "review-history"
        ? item.note
        : item.category === "accepted-deviation"
          ? `rule "${item.ruleId}"`
          : item.category === "suppression"
            ? `${item.fingerprintFile} / ${item.fingerprintRuleId ?? item.fingerprintCategory}`
            : `${item.exceptionKind}${item.ruleId ? ` (${item.ruleId})` : ""}`;
  return `[${item.id}] (${item.status}, ${item.confidence}) ${item.category}: ${detail}`;
}

export function registerMemoryCommand(program: Command) {
  const memory = program.command("memory").description("Inspect and manage Repository Memory");

  memory
    .command("list")
    .description("List every remembered item for this repository")
    .option("--status <status>", "Filter by lifecycle status")
    .action(async (options, command) => {
      const logger = command.logger as CliLogger;
      const cwd = program.opts().cwd as string;
      try {
        const store = loadMemoryStore(cwd);
        const items = options.status
          ? store.items.filter((i) => i.status === options.status)
          : store.items;
        if (logger.isJson) {
          logger.json(items);
        } else {
          logger.info(`${items.length} memory item(s).`);
          for (const item of items) {
            logger.log(`- ${describeItem(item)}`);
          }
        }
      } catch (err) {
        logger.error("Failed to list repository memory.", err);
        process.exit(1);
      }
    });

  const transitionCommand =
    (
      apply: (
        store: ReturnType<typeof loadMemoryStore>,
        id: string,
        actor: string,
        comment?: string,
      ) => ReturnType<typeof loadMemoryStore>,
      verb: string,
    ) =>
    async (id: string, options: { comment?: string }, command: { logger: CliLogger }) => {
      const logger = command.logger as CliLogger;
      const cwd = program.opts().cwd as string;
      try {
        const store = loadMemoryStore(cwd);
        const updated = apply(store, id, "cli", options.comment);
        saveMemoryStore(updated);
        const item = updated.items.find((i) => i.id === id);
        if (logger.isJson) {
          logger.json({ status: "success", item });
        } else {
          logger.success(`Memory item ${id} ${verb}${item ? ` — now "${item.status}"` : ""}.`);
        }
      } catch (err) {
        logger.error(`Failed to ${verb} memory item ${id}.`, err);
        process.exit(1);
      }
    };

  memory
    .command("suggest <id>")
    .description("Surface a detected item for review (detected -> suggested)")
    .action(async (id, options, command) =>
      transitionCommand(suggestMemory, "suggested")(id, options, command),
    );

  memory
    .command("confirm <id>")
    .description(
      "Confirm a suggested item — the only path to user-confirmed confidence (suggested -> confirmed)",
    )
    .option("--comment <text>", "Why this is being confirmed")
    .action(async (id, options, command) =>
      transitionCommand(confirmMemory, "confirmed")(id, options, command),
    );

  memory
    .command("activate <id>")
    .description("Start applying a confirmed item to future reviews (confirmed -> active)")
    .action(async (id, options, command) =>
      transitionCommand(activateMemory, "activated")(id, options, command),
    );

  memory
    .command("reject <id>")
    .description("Reject a detected/suggested item — it was never true (-> archived)")
    .option("--comment <text>", "Why this is being rejected")
    .action(async (id, options, command) =>
      transitionCommand(rejectMemory, "rejected")(id, options, command),
    );

  memory
    .command("deprecate <id>")
    .description("Stop applying an active item without deleting its history (active -> deprecated)")
    .option("--comment <text>", "Why this is being deprecated")
    .action(async (id, options, command) =>
      transitionCommand(deprecateMemory, "deprecated")(id, options, command),
    );

  memory
    .command("archive <id>")
    .description("Archive an item permanently — kept for audit, never consulted again")
    .option("--comment <text>", "Why this is being archived")
    .action(async (id, options, command) =>
      transitionCommand(archiveMemory, "archived")(id, options, command),
    );

  memory
    .command("remove <id>")
    .description("Delete an item entirely — distinct from archiving, which keeps a record")
    .action(async (id, _options, command) => {
      const logger = command.logger as CliLogger;
      const cwd = program.opts().cwd as string;
      try {
        const store = loadMemoryStore(cwd);
        const updated = removeMemoryItem(store, id);
        saveMemoryStore(updated);
        if (logger.isJson) {
          logger.json({ status: "success" });
        } else {
          logger.success(`Memory item ${id} removed.`);
        }
      } catch (err) {
        logger.error(`Failed to remove memory item ${id}.`, err);
        process.exit(1);
      }
    });
}
