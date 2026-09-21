import { describe, expect, it } from "bun:test";
import { detectorFor } from "./detectors.js";

describe("RULE_DETECTORS", () => {
  it("detects `any` usage only in TS files", () => {
    const detector = detectorFor("no-explicit-any");
    expect(detector).toBeDefined();
    const hits = detector?.({
      file: "src/foo.ts",
      content: "function f(x: any) {\n  return x;\n}",
      lineNumbers: undefined,
    });
    expect(hits).toHaveLength(1);
    expect(hits?.[0]?.lines).toEqual({ start: 1, end: 1 });

    const jsHits = detector?.({
      file: "src/foo.js",
      content: "function f(x) { return x; }",
      lineNumbers: undefined,
    });
    expect(jsHits).toHaveLength(0);
  });

  it("maps hits back to real line numbers via lineNumbers", () => {
    const detector = detectorFor("no-var");
    const hits = detector?.({
      file: "src/foo.js",
      content: "var a = 1;",
      lineNumbers: [42],
    });
    expect(hits?.[0]?.lines).toEqual({ start: 42, end: 42 });
  });

  it("flags eval() and setTimeout with a string argument", () => {
    const detector = detectorFor("no-eval");
    expect(
      detector?.({ file: "a.js", content: "eval(userInput)", lineNumbers: undefined }),
    ).toHaveLength(1);
    expect(
      detector?.({
        file: "a.js",
        content: 'setTimeout("doSomething()", 100)',
        lineNumbers: undefined,
      }),
    ).toHaveLength(1);
  });

  it("flags hardcoded secrets regardless of file extension", () => {
    const detector = detectorFor("no-hardcoded-secrets");
    const hits = detector?.({
      file: "config.yaml",
      content: 'api_key: "sk_live_1234567890abcdef"',
      lineNumbers: undefined,
    });
    expect(hits).toHaveLength(1);
  });

  it("has no detector for rules that require type/structural info", () => {
    expect(detectorFor("no-floating-promises")).toBeUndefined();
    expect(detectorFor("require-await")).toBeUndefined();
    expect(detectorFor("consistent-type-imports")).toBeUndefined();
  });
});

describe("Epic 16B detector expansion", () => {
  it("detects dangerouslySetInnerHTML only in tsx/jsx files", () => {
    const detector = detectorFor("no-dangerously-set-inner-html");
    expect(
      detector?.({
        file: "a.tsx",
        content: "<div dangerouslySetInnerHTML={{__html: x}} />",
        lineNumbers: undefined,
      }),
    ).toHaveLength(1);
    expect(
      detector?.({ file: "a.ts", content: "dangerouslySetInnerHTML", lineNumbers: undefined }),
    ).toHaveLength(0);
  });

  it("detects v-html only in .vue files", () => {
    const detector = detectorFor("use-v-html-carefully");
    expect(
      detector?.({ file: "a.vue", content: '<div v-html="raw"></div>', lineNumbers: undefined }),
    ).toHaveLength(1);
    expect(
      detector?.({ file: "a.ts", content: 'v-html="raw"', lineNumbers: undefined }),
    ).toHaveLength(0);
  });

  it("detects .unwrap()/.expect() only in .rs files", () => {
    const detector = detectorFor("no-unwrap-expect");
    expect(
      detector?.({ file: "a.rs", content: "let x = maybe.unwrap();", lineNumbers: undefined }),
    ).toHaveLength(1);
    expect(
      detector?.({
        file: "a.rs",
        content: 'let x = maybe.expect("boom");',
        lineNumbers: undefined,
      }),
    ).toHaveLength(1);
    expect(
      detector?.({ file: "a.ts", content: "maybe.unwrap()", lineNumbers: undefined }),
    ).toHaveLength(0);
  });

  it("detects unsafe blocks/functions only in .rs files", () => {
    const detector = detectorFor("no-unsafe-blocks");
    expect(
      detector?.({ file: "a.rs", content: "unsafe { ptr.read() }", lineNumbers: undefined }),
    ).toHaveLength(1);
    expect(
      detector?.({ file: "a.rs", content: "unsafe fn danger() {}", lineNumbers: undefined }),
    ).toHaveLength(1);
  });

  it("detects Rust's panic! macro and Go's panic() call, distinguished by extension", () => {
    const detector = detectorFor("no-panic");
    expect(
      detector?.({ file: "a.rs", content: 'panic!("bad")', lineNumbers: undefined }),
    ).toHaveLength(1);
    expect(
      detector?.({ file: "a.go", content: 'panic("bad")', lineNumbers: undefined }),
    ).toHaveLength(1);
    // Rust syntax shouldn't match against a .go file and vice versa.
    expect(
      detector?.({ file: "a.go", content: 'panic!("bad")', lineNumbers: undefined }),
    ).toHaveLength(0);
    expect(
      detector?.({ file: "a.ts", content: 'panic("bad")', lineNumbers: undefined }),
    ).toHaveLength(0);
  });
});
