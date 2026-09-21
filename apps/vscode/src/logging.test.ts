import { describe, expect, it } from "vitest";
import type { LogLevel } from "./config/settings.js";
import { Logger } from "./logging.js";

function logger(level: LogLevel) {
  const lines: string[] = [];
  let current = level;
  const instance = new Logger(
    { appendLine: (line) => lines.push(line) },
    () => current,
    () => new Date("2026-01-02T03:04:05.000Z"),
  );
  return { instance, lines, set: (next: LogLevel) => (current = next) };
}

const write = (l: Logger) => {
  l.error("e");
  l.info("i");
  l.debug("d");
};

describe("Logger", () => {
  it("keeps what the level allows and nothing below it", () => {
    const off = logger("off");
    write(off.instance);
    expect(off.lines).toEqual([]);

    const errors = logger("error");
    write(errors.instance);
    expect(errors.lines.map((line) => line.split(" ")[1])).toEqual(["ERROR"]);

    const info = logger("info");
    write(info.instance);
    expect(info.lines).toHaveLength(2);

    const debug = logger("debug");
    write(debug.instance);
    expect(debug.lines).toHaveLength(3);
  });

  it("follows the setting as it changes, without being recreated", () => {
    const { instance, lines, set } = logger("error");
    instance.info("hidden");
    set("info");
    instance.info("shown");
    expect(lines).toEqual(["2026-01-02T03:04:05.000Z INFO  shown"]);
  });

  it("puts fields on the line", () => {
    const { instance, lines } = logger("info");
    instance.info("review done", { findings: 3, ok: true, scope: "workspace" });
    expect(lines[0]).toBe(
      '2026-01-02T03:04:05.000Z INFO  review done findings=3 ok=true scope="workspace"',
    );
  });

  it("never lets a long value hold a file", () => {
    const { instance, lines } = logger("info");
    instance.error("failed", { output: "x".repeat(5_000) });
    expect(lines[0]?.length).toBeLessThan(400);
    expect(lines[0]).toContain("…");
  });
});
