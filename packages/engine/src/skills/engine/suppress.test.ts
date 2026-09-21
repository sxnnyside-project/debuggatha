import { describe, expect, it } from "bun:test";
import { lex } from "./lex.js";
import { findDirective, parseDirectives } from "./suppress.js";

const directivesOf = (source: string, file = "a.ts") => parseDirectives(lex(source, file).comments);

describe("inline suppression", () => {
  it("covers the line it sits on, for the rules it names, with its reason", () => {
    const [directive] = directivesOf("risky(); // debuggatha-ignore no-eval -- sandboxed loader");
    expect(directive).toEqual({
      scope: "line",
      line: 1,
      ruleIds: ["no-eval"],
      reason: "sandboxed loader",
    });
  });

  it("covers only the next line with -next-line", () => {
    const directives = directivesOf(
      "// debuggatha-ignore-next-line eqeqeq\nif (a == b) {}\nif (c == d) {}",
    );
    expect(findDirective(directives, 2, "eqeqeq")).toBeDefined();
    expect(findDirective(directives, 3, "eqeqeq")).toBeUndefined();
    expect(findDirective(directives, 1, "eqeqeq")).toBeUndefined();
  });

  it("covers the whole file with -file", () => {
    const directives = directivesOf(
      "// debuggatha-ignore-file no-explicit-any -- generated\nlet a: any;",
    );
    expect(findDirective(directives, 2, "no-explicit-any")).toBeDefined();
    expect(findDirective(directives, 99, "no-explicit-any")).toBeDefined();
  });

  it("does not cover a rule it did not name", () => {
    const directives = directivesOf("x(); // debuggatha-ignore no-eval");
    expect(findDirective(directives, 1, "no-eval")).toBeDefined();
    expect(findDirective(directives, 1, "no-var")).toBeUndefined();
  });

  it("covers every rule when none is named, or when * is", () => {
    for (const source of ["x(); // debuggatha-ignore", "x(); // debuggatha-ignore * -- why"]) {
      const directives = directivesOf(source);
      expect(findDirective(directives, 1, "anything")).toBeDefined();
    }
  });

  it("takes several rule ids separated by commas or spaces", () => {
    const [directive] = directivesOf("x(); // debuggatha-ignore no-eval, no-var eqeqeq -- legacy");
    expect(directive?.ruleIds).toEqual(["no-eval", "no-var", "eqeqeq"]);
    expect(directive?.reason).toBe("legacy");
  });

  it("works in hash-comment languages and block comments", () => {
    expect(directivesOf("x = eval(y)  # debuggatha-ignore no-eval -- why", "a.py")).toHaveLength(1);
    expect(directivesOf("x(); /* debuggatha-ignore no-eval -- why */")[0]?.reason).toBe("why");
  });

  it("ignores the words inside a string, which is not a comment", () => {
    expect(directivesOf('const s = "debuggatha-ignore no-eval";')).toHaveLength(0);
  });
});
