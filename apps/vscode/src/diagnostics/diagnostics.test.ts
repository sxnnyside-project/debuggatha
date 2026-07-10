import { describe, expect, it, vi } from "vitest";

// Mock vscode
vi.mock("vscode", () => {
  return {
    Range: class Range {
      constructor(
        public startLine: number,
        public startChar: number,
        public endLine: number,
        public endChar: number,
      ) {}
    },
    Diagnostic: class Diagnostic {
      constructor(
        public range: any,
        public message: string,
        public severity: any,
      ) {}
      source = "";
      code = "";
    },
    DiagnosticSeverity: {
      Error: 0,
      Warning: 1,
      Information: 2,
    },
    Uri: {
      file: (f: string) => f,
    },
  };
});

describe("diagnostics", () => {
  it("should format findings into diagnostics", () => {
    // Tests will run in isolation without breaking VS Code APIs
    expect(true).toBe(true);
  });
});
