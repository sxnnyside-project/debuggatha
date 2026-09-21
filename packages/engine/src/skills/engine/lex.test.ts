import { describe, expect, it } from "bun:test";
import { lex } from "./lex.js";

describe("lex", () => {
  it("keeps the source layout: same length, same line breaks", () => {
    const source = 'const a = "hello"; // note\n/* block\n   text */\nconst b = `x\ny`;\n';
    const { masked } = lex(source, "a.ts");
    expect(masked.length).toBe(source.length);
    expect(masked.split("\n").length).toBe(source.split("\n").length);
    for (let i = 0; i < source.length; i += 1) {
      if (source[i] === "\n") expect(masked[i]).toBe("\n");
    }
  });

  it("blanks the inside of strings and comments but keeps code and quotes", () => {
    const { masked } = lex('if (x == 1) { eval("y == 2"); } // == 3', "a.ts");
    expect(masked).toContain("if (x == 1)");
    expect(masked).toContain("eval(");
    expect(masked).not.toContain("y == 2");
    expect(masked).not.toContain("== 3");
    expect(masked).toContain(`"${" ".repeat("y == 2".length)}"`);
  });

  it("finds string literals with their value and position", () => {
    const source = 'const password = "hunter22222";';
    const { strings } = lex(source, "a.ts");
    expect(strings).toHaveLength(1);
    expect(strings[0]?.value).toBe("hunter22222");
    expect(source.slice(strings[0]?.start, strings[0]?.end)).toBe('"hunter22222"');
  });

  it("does not let an escaped quote end a string early", () => {
    const { strings, masked } = lex('const s = "say \\"eval(x)\\" now"; eval(y);', "a.ts");
    expect(strings).toHaveLength(1);
    expect(masked.match(/eval\(/g)).toHaveLength(1);
  });

  it("treats a JavaScript regex literal as text, so a quote inside it does not start a string", () => {
    const { strings, masked } = lex("const re = /[\"'] == x/; var y = 1;", "a.js");
    expect(strings).toHaveLength(0);
    expect(masked).toContain("var y = 1;");
    expect(masked).not.toContain("== x");
  });

  it("still reads division as division", () => {
    const { masked } = lex("const half = total / 2; const b = a == c;", "a.ts");
    expect(masked).toContain("a == c");
  });

  it("handles template literals across lines", () => {
    const { masked, strings } = lex("const t = `a\nvar b`;\nvar c = 1;", "a.ts");
    expect(strings).toHaveLength(1);
    expect(masked).not.toContain("var b");
    expect(masked).toContain("var c = 1;");
  });

  it("collects comments per line, including block comment lines", () => {
    const { comments } = lex("a(); // first\n/* second\n third */\n", "a.ts");
    expect(comments.map((c) => [c.line, c.text.trim()])).toEqual([
      [1, "first"],
      [2, "second"],
      [3, "third"],
    ]);
  });

  it("uses hash comments for Python and Ruby, and triple quotes for Python and Kotlin", () => {
    expect(lex("x = 1  # eval(y)", "a.py").masked).not.toContain("eval");
    expect(lex('s = """eval(y)\nmore"""\nz = 2', "a.py").masked).toContain("z = 2");
    expect(lex('val s = """eval(y)"""\nval z = 2', "a.kt").masked).not.toContain("eval");
  });

  it("does not mistake a Rust lifetime for a string", () => {
    const { strings, masked } = lex("fn f<'a>(x: &'a str) { x.unwrap() }", "a.rs");
    expect(strings).toHaveLength(0);
    expect(masked).toContain("x.unwrap()");
  });

  it("returns an unknown language untouched", () => {
    const source = "<div v-html='x'></div> // not a comment here";
    expect(lex(source, "a.vue")).toEqual({ masked: source, strings: [], comments: [] });
  });

  it("survives an unterminated string and an unterminated block comment", () => {
    expect(() => lex('const s = "oops\nvar x = 1;', "a.ts")).not.toThrow();
    expect(lex("a(); /* never closed", "a.ts").masked.length).toBe("a(); /* never closed".length);
  });
});
