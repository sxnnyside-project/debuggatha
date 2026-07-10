import { describe, expect, it } from "vitest";
import { createCli } from "./index.js";

describe("Debuggatha CLI", () => {
  it("registers all expected commands", () => {
    const cli = createCli();
    const commandNames = cli.commands.map((c) => c.name());

    expect(commandNames).toContain("init");
    expect(commandNames).toContain("review");
    expect(commandNames).toContain("findings");
    expect(commandNames).toContain("repository");
    expect(commandNames).toContain("packs");
    expect(commandNames).toContain("policies");
    expect(commandNames).toContain("doctor");
  });

  it("parses global options correctly", () => {
    const cli = createCli();
    cli.exitOverride();
    cli.configureOutput({
      writeOut: () => {},
      writeErr: () => {},
    });
    // Override action to prevent side-effects during test
    cli.commands.find((c) => c.name() === "doctor")!.action(() => {});

    cli.parse(["node", "test", "doctor", "--json", "--verbose", "--quiet", "--cwd", "/tmp"]);

    const opts = cli.opts();
    expect(opts.json).toBe(true);
    expect(opts.verbose).toBe(true);
    expect(opts.quiet).toBe(true);
    expect(opts.cwd).toBe("/tmp");
  });

  it("shows help if executed without command", () => {
    const cli = createCli();
    cli.exitOverride();
    let helpOutput = "";
    cli.configureOutput({
      writeOut: (str) => {
        helpOutput += str;
      },
      writeErr: () => {},
    });

    expect(() => cli.parse(["node", "test", "--help"])).toThrow(); // throws CommanderError for exit
    expect(helpOutput).toContain("Usage: debuggatha");
    expect(helpOutput).toContain("Commands:");
  });
});
