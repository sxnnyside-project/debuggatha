import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Severity } from "@debuggatha/core";
import { IGNORED_DIRS } from "../scan-scope/index.js";
import { toRepositoryPath } from "./paths.js";
import { jsonFrom, parseSarif, severityFromLevel } from "./sarif.js";
import type { AnalyzerAdapter, AnalyzerFinding, ParseContext } from "./types.js";

/*
 * One adapter per tool. Each runs the tool as its own process and reads what it
 * prints; none of them bundles, copies, or redistributes the tool or its rules.
 * Commands were checked against the real tools, and the parsers against output
 * recorded from them (`fixtures/`).
 */

function categoryFrom(table: readonly [RegExp, string][], fallback: string) {
  return (ruleId: string) => table.find(([pattern]) => pattern.test(ruleId))?.[1] ?? fallback;
}

function withLocation(
  finding: Omit<AnalyzerFinding, "file" | "line" | "column" | "endLine" | "endColumn">,
  context: ParseContext,
  file: string | undefined,
  position: { line?: number; column?: number; endLine?: number; endColumn?: number },
): AnalyzerFinding | undefined {
  const relativeFile = file ? toRepositoryPath(context.rootDir, file) : undefined;
  if (!relativeFile) return undefined;
  return {
    ...finding,
    file: relativeFile,
    ...(position.line !== undefined ? { line: position.line } : {}),
    ...(position.column !== undefined ? { column: position.column } : {}),
    ...(position.endLine !== undefined ? { endLine: position.endLine } : {}),
    ...(position.endColumn !== undefined ? { endColumn: position.endColumn } : {}),
  };
}

function compact<T>(items: (T | undefined)[]): T[] {
  return items.filter((item): item is T => item !== undefined);
}

const scoped = (context: { files: readonly string[] | undefined }, fallback: string[]) =>
  context.files ? [...context.files] : fallback;

/* ------------------------------------------------------------------ gitleaks */

const CONFIG_FILES = [".gitleaks.toml", "gitleaks.toml"];

export const gitleaks: AnalyzerAdapter = {
  id: "gitleaks",
  name: "Gitleaks",
  license: "MIT",
  homepage: "https://github.com/gitleaks/gitleaks",
  covers: "secrets in any language",
  trust: "safe",
  applies: () => true,
  binaries: { local: [], names: ["gitleaks"] },
  versionArgs: ["version"],
  okExitCodes: [0],
  supersedes: ["no-hardcoded-secrets"],
  command({ rootDir }) {
    const args = [
      "dir",
      ".",
      "--no-banner",
      "--log-level",
      "error",
      "--redact",
      "--report-format",
      "sarif",
      "--report-path",
      "-",
      "--exit-code",
      "0",
    ];
    if (CONFIG_FILES.some((name) => existsSync(join(rootDir, name)))) return { args };
    // No repository config: scan what the project owns. `dir` does not read `.gitignore`.
    const directory = mkdtempSync(join(tmpdir(), "debuggatha-gitleaks-"));
    const config = join(directory, "gitleaks.toml");
    const skipped = [...IGNORED_DIRS].map((name) => name.replace(/[.\\+*?()|[\]{}^$]/g, "\\$&"));
    writeFileSync(
      config,
      `[extend]\nuseDefault = true\n\n[[allowlists]]\npaths = ['''(^|/)(${skipped.join("|")})/''']\n`,
    );
    return {
      args: [...args, "--config", config],
      cleanup: () => rmSync(directory, { recursive: true, force: true }),
    };
  },
  parse: (stdout, context) =>
    parseSarif(stdout, context, {
      category: () => "security",
      severity: () => "critical",
      sensitive: true,
    }),
};

/* --------------------------------------------------------------------- biome */

interface BiomeDiagnostic {
  severity?: string;
  message?: string;
  category?: string;
  location?: {
    path?: string | { file?: string };
    start?: { line?: number; column?: number };
    end?: { line?: number; column?: number };
  };
}

const BIOME_CATEGORY = categoryFrom(
  [
    [/^lint\/security\//, "security"],
    [/^lint\/(correctness|suspicious)\//, "reliability"],
    [/^lint\/performance\//, "performance"],
    [/^lint\/a11y\//, "accessibility"],
  ],
  "maintainability",
);

/** `lint/suspicious/noExplicitAny` is documented at `.../rules/no-explicit-any`. */
function biomeDocs(category: string): string {
  const name = (category.split("/").pop() ?? "").replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);
  return `https://biomejs.dev/linter/rules/${name}`;
}

export const biome: AnalyzerAdapter = {
  id: "biome",
  name: "Biome",
  license: "MIT OR Apache-2.0",
  homepage: "https://biomejs.dev",
  covers: "JavaScript, TypeScript, JSON, CSS",
  trust: "safe",
  applies: ({ has }) => has(/(^|\/)biome\.jsonc?$/),
  binaries: { local: ["node_modules/.bin/biome"], names: ["biome"] },
  versionArgs: ["--version"],
  okExitCodes: [0, 1],
  fileFilter: /\.(m|c)?[jt]sx?$|\.json$|\.css$/,
  equivalentTo: {
    "lint/suspicious/noDoubleEquals": "eqeqeq",
    "lint/suspicious/noExplicitAny": "no-explicit-any",
    "lint/security/noGlobalEval": "no-eval",
    "lint/style/noVar": "no-var",
    "lint/correctness/noVar": "no-var",
  },
  command: (context) => ({
    args: [
      "lint",
      "--reporter=json",
      "--colors=off",
      "--max-diagnostics=none",
      ...scoped(context, ["."]),
    ],
  }),
  parse(stdout, context) {
    const { diagnostics = [] } = jsonFrom(stdout) as { diagnostics?: BiomeDiagnostic[] };
    return compact(
      diagnostics.map((diagnostic) => {
        const path = diagnostic.location?.path;
        const file = typeof path === "string" ? path : path?.file;
        const ruleId = diagnostic.category ?? "biome";
        const severity: Severity =
          diagnostic.severity === "error"
            ? "medium"
            : diagnostic.severity === "information"
              ? "informational"
              : "low";
        return withLocation(
          {
            ruleId,
            message: diagnostic.message ?? ruleId,
            severity,
            category: BIOME_CATEGORY(ruleId),
            ...(ruleId.startsWith("lint/") ? { url: biomeDocs(ruleId) } : {}),
          },
          context,
          file,
          {
            ...(diagnostic.location?.start?.line !== undefined
              ? { line: diagnostic.location.start.line }
              : {}),
            ...(diagnostic.location?.start?.column !== undefined
              ? { column: diagnostic.location.start.column }
              : {}),
            ...(diagnostic.location?.end?.line !== undefined
              ? { endLine: diagnostic.location.end.line }
              : {}),
            ...(diagnostic.location?.end?.column !== undefined
              ? { endColumn: diagnostic.location.end.column }
              : {}),
          },
        );
      }),
    );
  },
};

/* -------------------------------------------------------------------- eslint */

interface EslintFile {
  filePath: string;
  messages: {
    ruleId: string | null;
    severity: number;
    message: string;
    line?: number;
    column?: number;
    endLine?: number;
    endColumn?: number;
  }[];
}

const ESLINT_CATEGORY = categoryFrom(
  [
    [/^(no-eval|no-implied-eval|no-new-func|no-script-url|security\/)/, "security"],
    [
      /^(eqeqeq|no-undef|no-unused-vars|no-dupe|no-unreachable|no-cond-assign|use-isnan|valid-typeof|react-hooks\/)/,
      "reliability",
    ],
    [/^(jsx-a11y\/)/, "accessibility"],
  ],
  "maintainability",
);

export const eslint: AnalyzerAdapter = {
  id: "eslint",
  name: "ESLint",
  license: "MIT",
  homepage: "https://eslint.org",
  covers: "JavaScript, TypeScript",
  // An ESLint config is JavaScript, and plugins are packages the project chose to run.
  trust: "runs-project-code",
  applies: ({ has }) => has(/(^|\/)(eslint\.config\.[cm]?[jt]s|\.eslintrc(\.[a-z]+)?)$/),
  binaries: { local: ["node_modules/.bin/eslint"], names: ["eslint"] },
  versionArgs: ["--version"],
  okExitCodes: [0, 1],
  fileFilter: /\.(m|c)?[jt]sx?$/,
  equivalentTo: {
    "no-var": "no-var",
    eqeqeq: "eqeqeq",
    "no-eval": "no-eval",
    "no-implied-eval": "no-implied-eval",
    "no-console": "avoid-print",
  },
  command: (context) => ({
    args: [
      "--format",
      "json",
      "--no-warn-ignored",
      "--no-error-on-unmatched-pattern",
      ...scoped(context, ["."]),
    ],
  }),
  parse(stdout, context) {
    const files = JSON.parse(stdout.slice(stdout.indexOf("["))) as EslintFile[];
    return compact(
      files.flatMap((entry) =>
        entry.messages.map((message) => {
          const ruleId = message.ruleId ?? "parse-error";
          return withLocation(
            {
              ruleId,
              message: message.message,
              severity: message.severity >= 2 ? "medium" : "low",
              category: ESLINT_CATEGORY(ruleId),
              ...(message.ruleId && !message.ruleId.includes("/")
                ? { url: `https://eslint.org/docs/latest/rules/${message.ruleId}` }
                : {}),
            },
            context,
            entry.filePath,
            {
              ...(message.line !== undefined ? { line: message.line } : {}),
              ...(message.column !== undefined ? { column: message.column } : {}),
              ...(message.endLine !== undefined ? { endLine: message.endLine } : {}),
              ...(message.endColumn !== undefined ? { endColumn: message.endColumn } : {}),
            },
          );
        }),
      ),
    );
  },
};

/* ---------------------------------------------------------------------- ruff */

interface RuffDiagnostic {
  code: string | null;
  message: string;
  filename: string;
  url?: string | null;
  location?: { row: number; column: number };
  end_location?: { row: number; column: number };
}

export const ruff: AnalyzerAdapter = {
  id: "ruff",
  name: "Ruff",
  license: "MIT",
  homepage: "https://docs.astral.sh/ruff",
  covers: "Python",
  trust: "safe",
  applies: ({ has }) => has(/\.pyi?$/),
  binaries: { local: [".venv/bin/ruff", "venv/bin/ruff"], names: ["ruff"] },
  versionArgs: ["--version"],
  okExitCodes: [0],
  fileFilter: /\.pyi?$/,
  // `--no-cache` keeps Ruff from writing `.ruff_cache` into the repository.
  command: (context) => ({
    args: [
      "check",
      "--output-format",
      "json",
      "--exit-zero",
      "--no-cache",
      ...scoped(context, ["."]),
    ],
  }),
  parse(stdout, context) {
    const diagnostics = JSON.parse(stdout) as RuffDiagnostic[];
    return compact(
      diagnostics.map((diagnostic) => {
        const code = diagnostic.code ?? "syntax-error";
        const security = code.startsWith("S");
        const severity: Severity = /^(F|E9|S|B)/.test(code) || !diagnostic.code ? "medium" : "low";
        return withLocation(
          {
            ruleId: code,
            message: diagnostic.message,
            severity,
            category: security
              ? "security"
              : /^(F|E9|B)/.test(code) || !diagnostic.code
                ? "reliability"
                : "maintainability",
            ...(diagnostic.url ? { url: diagnostic.url } : {}),
          },
          context,
          diagnostic.filename,
          {
            ...(diagnostic.location
              ? { line: diagnostic.location.row, column: diagnostic.location.column }
              : {}),
            ...(diagnostic.end_location
              ? { endLine: diagnostic.end_location.row, endColumn: diagnostic.end_location.column }
              : {}),
          },
        );
      }),
    );
  },
};

/* -------------------------------------------------------------------- ktlint */

export const ktlint: AnalyzerAdapter = {
  id: "ktlint",
  name: "ktlint",
  license: "MIT",
  homepage: "https://pinterest.github.io/ktlint",
  covers: "Kotlin style",
  trust: "safe",
  applies: ({ has }) => has(/\.kts?$/),
  binaries: { local: [], names: ["ktlint"] },
  versionArgs: ["--version"],
  okExitCodes: [0, 1],
  fileFilter: /\.kts?$/,
  command: (context) => ({
    args: [
      "--reporter=sarif",
      ...scoped(context, ["**/*.kt", "**/*.kts", "!**/build/**", "!**/.gradle/**"]),
    ],
  }),
  parse: (stdout, context) =>
    parseSarif(stdout, context, {
      category: () => "maintainability",
      severity: () => "low",
    }),
};

/* -------------------------------------------------------------------- detekt */

const DETEKT_CONFIGS = [
  "detekt.yml",
  "config/detekt/detekt.yml",
  "config/detekt.yml",
  "detekt-config.yml",
];

const DETEKT_CATEGORY = categoryFrom(
  [
    [/^detekt\.(potential-bugs|coroutines)\./, "reliability"],
    [/^detekt\.performance\./, "performance"],
  ],
  "maintainability",
);

export const detekt: AnalyzerAdapter = {
  id: "detekt",
  name: "detekt",
  license: "Apache-2.0",
  homepage: "https://detekt.dev",
  covers: "Kotlin code smells and bugs",
  trust: "safe",
  // Without a config detekt would judge the project by its defaults; the repository has to have chosen it.
  applies: ({ has }) => has(/\.kts?$/) && has(/(^|\/)(detekt[\w-]*\.ya?ml)$/),
  binaries: { local: [], names: ["detekt-cli", "detekt"] },
  versionArgs: ["--version"],
  // 2: issues found. 0 and 2 both mean the report is valid.
  okExitCodes: [0, 2],
  fileFilter: /\.kts?$/,
  command({ rootDir, files }) {
    const config = DETEKT_CONFIGS.find((name) => existsSync(join(rootDir, name)));
    return {
      args: [
        "--input",
        (files ? [...files] : ["."]).join(","),
        ...(config ? ["--config", config, "--build-upon-default-config"] : []),
        "--report",
        "sarif:/dev/stdout",
      ],
    };
  },
  parse: (stdout, context) =>
    parseSarif(stdout, context, {
      category: (ruleId) => DETEKT_CATEGORY(ruleId),
      severity: (level) => severityFromLevel(level === "error" ? "error" : "warning"),
    }),
};

/* ------------------------------------------------------------------- phpstan */

interface PhpstanReport {
  files?: Record<
    string,
    { messages: { message: string; line?: number | null; identifier?: string; tip?: string }[] }
  >;
  /** Problems with the run itself (bad config, a crashed worker), in a list when PHPStan finished. */
  errors?: string[] | number;
  general_errors?: string[];
  result?: string;
}

const PHPSTAN_CONFIGS = ["phpstan.neon", "phpstan.neon.dist", "phpstan.dist.neon"];
const PHP_SOURCE_DIRS = ["src", "app", "lib"];

export const phpstan: AnalyzerAdapter = {
  id: "phpstan",
  name: "PHPStan",
  license: "MIT",
  homepage: "https://phpstan.org",
  covers: "PHP type and logic errors",
  // PHPStan loads the project's autoloader and bootstrap files.
  trust: "runs-project-code",
  applies: ({ has }) =>
    has(/\.php$/) && has(/(^|\/)(composer\.json|phpstan\.(dist\.)?neon(\.dist)?)$/),
  binaries: { local: ["vendor/bin/phpstan"], names: ["phpstan"] },
  versionArgs: ["--version"],
  okExitCodes: [0, 1],
  fileFilter: /\.php$/,
  command({ rootDir, files }) {
    const configured = PHPSTAN_CONFIGS.some((name) => existsSync(join(rootDir, name)));
    const directories = PHP_SOURCE_DIRS.filter((name) => existsSync(join(rootDir, name)));
    return {
      args: [
        "analyse",
        "--error-format=json",
        "--no-progress",
        "--no-interaction",
        // PHP's own default of 128M is too little for a real application.
        "--memory-limit=1G",
        // Without a config there is no level; 0 only reports what is certainly wrong.
        ...(configured ? [] : ["--level=0"]),
        ...(files ? [...files] : configured ? [] : directories.length > 0 ? directories : ["."]),
      ],
    };
  },
  parse(stdout, context) {
    const report = jsonFrom(stdout) as PhpstanReport;
    // A run that did not finish says nothing about the code; "no findings" would be a lie.
    const problems = [
      ...(Array.isArray(report.errors) ? report.errors : []),
      ...(report.general_errors ?? []),
    ];
    if (problems.length > 0) {
      throw new Error(`PHPStan could not finish: ${problems[0]?.split("\n")[0]}`);
    }
    if (report.result === "failed" && !report.files) {
      throw new Error("PHPStan reported a failed run without saying why");
    }
    return compact(
      Object.entries(report.files ?? {}).flatMap(([file, entry]) =>
        entry.messages.map((message) => {
          const ruleId = message.identifier ?? "phpstan";
          return withLocation(
            {
              ruleId,
              message: message.message,
              severity: "medium",
              category: "reliability",
              ...(message.identifier
                ? { url: `https://phpstan.org/error-identifiers/${message.identifier}` }
                : {}),
            },
            context,
            file,
            message.line ? { line: message.line } : {},
          );
        }),
      ),
    );
  },
};

/* ------------------------------------------------------------------- clippy */

interface CargoMessage {
  reason: string;
  message?: {
    level?: string;
    message?: string;
    code?: { code?: string } | null;
    children?: { message?: string }[];
    spans?: {
      file_name: string;
      line_start: number;
      line_end: number;
      column_start: number;
      column_end: number;
      is_primary: boolean;
    }[];
  };
}

const CLIPPY_CATEGORY = categoryFrom(
  [
    [
      /unwrap_used|expect_used|panic|indexing_slicing|unreachable|todo|unimplemented/,
      "reliability",
    ],
    [/^clippy::(perf|redundant_clone|useless_vec|needless_collect)/, "performance"],
  ],
  "maintainability",
);

export const clippy: AnalyzerAdapter = {
  id: "clippy",
  name: "Clippy",
  license: "MIT OR Apache-2.0",
  homepage: "https://doc.rust-lang.org/clippy",
  covers: "Rust",
  // Clippy compiles the crate: build scripts and procedural macros run, and missing dependencies are downloaded.
  trust: "runs-project-code",
  applies: ({ has }) => has(/(^|\/)Cargo\.toml$/),
  binaries: { local: [], names: ["cargo"] },
  prefix: ["clippy"],
  versionArgs: ["--version"],
  okExitCodes: [0, 101],
  command: ({ rootDir }) => ({
    args: [
      "--message-format=json",
      "--quiet",
      "--workspace",
      "--all-targets",
      // Build output goes to the temp directory, never into the repository's `target/`.
      "--target-dir",
      join(
        tmpdir(),
        `debuggatha-clippy-${createHash("sha256").update(rootDir).digest("hex").slice(0, 12)}`,
      ),
    ],
  }),
  parse(stdout, context) {
    const findings: (AnalyzerFinding | undefined)[] = [];
    for (const line of stdout.split("\n")) {
      if (!line.startsWith("{")) continue;
      let parsed: CargoMessage;
      try {
        parsed = JSON.parse(line) as CargoMessage;
      } catch {
        continue;
      }
      const message = parsed.message;
      if (parsed.reason !== "compiler-message" || !message?.code?.code) continue;
      if (message.level !== "warning" && message.level !== "error") continue;
      const span = message.spans?.find((candidate) => candidate.is_primary) ?? message.spans?.[0];
      if (!span) continue;
      const ruleId = message.code.code;
      const help = message.children?.map((child) => child.message ?? "").join(" ") ?? "";
      const url = /https:\/\/rust-lang\.github\.io\/[^\s`]+/.exec(help)?.[0];
      findings.push(
        withLocation(
          {
            ruleId,
            message: message.message ?? ruleId,
            severity: message.level === "error" ? "high" : "low",
            category: CLIPPY_CATEGORY(ruleId),
            ...(url ? { url } : {}),
          },
          context,
          span.file_name,
          {
            line: span.line_start,
            column: span.column_start,
            endLine: span.line_end,
            endColumn: span.column_end,
          },
        ),
      );
    }
    return compact(findings);
  },
};

/* ------------------------------------------------------------- osv-scanner */

export const osvScanner: AnalyzerAdapter = {
  id: "osv-scanner",
  name: "OSV-Scanner",
  license: "Apache-2.0",
  homepage: "https://google.github.io/osv-scanner",
  covers: "known vulnerabilities in dependencies",
  // It sends package names and versions to the OSV.dev API.
  trust: "network",
  applies: ({ has }) =>
    has(
      /(^|\/)(package-lock\.json|yarn\.lock|pnpm-lock\.yaml|bun\.lock|Cargo\.lock|composer\.lock|go\.sum|Gemfile\.lock|poetry\.lock|uv\.lock|requirements\.txt|gradle\.lockfile|pom\.xml)$/,
    ),
  binaries: { local: [], names: ["osv-scanner"] },
  versionArgs: ["--version"],
  // 1: vulnerabilities found. 128: nothing to scan.
  okExitCodes: [0, 1, 128],
  command: () => ({ args: ["scan", "source", "--format", "sarif", "--recursive", "."] }),
  parse: (stdout, context) =>
    stdout.trim() === "" ? [] : parseSarif(stdout, context, { category: () => "security" }),
};

export const ADAPTERS: readonly AnalyzerAdapter[] = [
  gitleaks,
  biome,
  eslint,
  ruff,
  ktlint,
  detekt,
  phpstan,
  clippy,
  osvScanner,
];
