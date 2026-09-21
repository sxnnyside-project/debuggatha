import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { ScanResult } from "../internal/scan-result.js";
import type { CriteriaProfile, CriteriaRule } from "../types.js";

/**
 * Criteria Resolution (not "Criteria Loading" — see CLAUDE.md vocabulary).
 *
 * Config files (JSON/INI-shaped ones) are parsed into real structured
 * rules. Markdown documentation that carries engineering rules in prose
 * (ARCHITECTURE.md, CONTRIBUTING.md, CLAUDE.md, *_CRITERIA.md) is
 * structured at heading granularity — each `##`/`###` section becomes one
 * rule whose description is the heading text. That is honest about what
 * this does NOT do: it does not understand the prose underneath a
 * heading, only that a rule-shaped section exists (see the package
 * README's "Risks" section). No LLM reasoning is used anywhere here.
 */

function eslintRules(json: Record<string, unknown>, file: string): CriteriaRule[] {
  const rules = (json.rules ?? {}) as Record<string, unknown>;
  return Object.keys(rules).map((ruleName) => ({
    id: `eslint:${ruleName}`,
    description: `ESLint rule "${ruleName}" configured`,
    source: { file, detail: JSON.stringify(rules[ruleName]) },
  }));
}

function biomeRules(json: Record<string, unknown>, file: string): CriteriaRule[] {
  const out: CriteriaRule[] = [];
  const linter = (json.linter ?? {}) as Record<string, unknown>;
  const linterRules = (linter.rules ?? {}) as Record<string, unknown>;

  for (const [category, categoryRules] of Object.entries(linterRules)) {
    if (category === "recommended") {
      out.push({
        id: "biome:linter.recommended",
        description: `Biome recommended lint rules: ${categoryRules}`,
        source: { file, detail: `linter.rules.recommended = ${categoryRules}` },
      });
      continue;
    }
    if (categoryRules && typeof categoryRules === "object") {
      for (const ruleName of Object.keys(categoryRules as Record<string, unknown>)) {
        out.push({
          id: `biome:${category}.${ruleName}`,
          description: `Biome rule "${category}.${ruleName}" configured`,
          source: {
            file,
            detail: JSON.stringify((categoryRules as Record<string, unknown>)[ruleName]),
          },
        });
      }
    }
  }

  const formatter = (json.formatter ?? {}) as Record<string, unknown>;
  for (const [key, value] of Object.entries(formatter)) {
    if (key === "enabled") continue;
    out.push({
      id: `biome:formatter.${key}`,
      description: `Biome formatter setting "${key}" = ${JSON.stringify(value)}`,
      source: { file, detail: `formatter.${key} = ${JSON.stringify(value)}` },
    });
  }

  return out;
}

function prettierRules(json: Record<string, unknown>, file: string): CriteriaRule[] {
  return Object.entries(json).map(([key, value]) => ({
    id: `prettier:${key}`,
    description: `Prettier setting "${key}" = ${JSON.stringify(value)}`,
    source: { file, detail: `${key} = ${JSON.stringify(value)}` },
  }));
}

function parseEditorConfig(content: string, file: string): CriteriaRule[] {
  const rules: CriteriaRule[] = [];
  let currentSection = "*";

  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#") || line.startsWith(";")) continue;

    const sectionMatch = line.match(/^\[(.+)\]$/);
    if (sectionMatch?.[1]) {
      currentSection = sectionMatch[1];
      continue;
    }

    const kvMatch = line.match(/^([\w.-]+)\s*=\s*(.+)$/);
    if (kvMatch?.[1] && kvMatch[2] !== undefined) {
      const [, key, value] = kvMatch;
      rules.push({
        id: `editorconfig:${currentSection}:${key}`,
        description: `EditorConfig "${key}" = "${value}" for ${currentSection}`,
        source: { file, detail: `[${currentSection}] ${key} = ${value}` },
      });
    }
  }

  return rules;
}

/** Best-effort flat `key = value` extraction — not a TOML parser. */
function parseFlatToml(content: string, file: string, idPrefix: string): CriteriaRule[] {
  const rules: CriteriaRule[] = [];

  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#") || line.startsWith("[")) continue;

    const match = line.match(/^([\w.-]+)\s*=\s*(.+)$/);
    if (match?.[1] && match[2] !== undefined) {
      const [, key, value] = match;
      rules.push({
        id: `${idPrefix}:${key}`,
        description: `${idPrefix} "${key}" = ${value}`,
        source: { file, detail: `${key} = ${value}` },
      });
    }
  }

  return rules;
}

function extractHeadingRules(content: string, file: string): CriteriaRule[] {
  const rules: CriteriaRule[] = [];
  const headingRegex = /^(##|###)\s+(.+)$/gm;
  let match: RegExpExecArray | null = headingRegex.exec(content);
  let index = 0;

  while (match !== null) {
    const heading = match[2]?.trim();
    if (heading) {
      index += 1;
      rules.push({
        id: `${file}:${index}`,
        description: heading,
        source: { file, detail: `Section "${heading}"` },
      });
    }
    match = headingRegex.exec(content);
  }

  return rules;
}

const RULE_SHAPED_DOCS = [
  "ARCHITECTURE.md",
  "CONTRIBUTING.md",
  "CLAUDE.md",
  "UX_CRITERIA.md",
  "UI_CRITERIA.md",
  "DX_CRITERIA.md",
];

function tryParseJson(content: string): Record<string, unknown> | undefined {
  try {
    const parsed = JSON.parse(content);
    return parsed && typeof parsed === "object" ? (parsed as Record<string, unknown>) : undefined;
  } catch {
    return undefined;
  }
}

export function scanCriteria(
  rootDir: string,
  rootEntries: readonly string[],
): ScanResult<CriteriaProfile> {
  const entries = new Set(rootEntries);
  const filesRead: string[] = [];
  const rules: CriteriaRule[] = [];
  const sources = new Set<string>();

  const readFile = (file: string): string => {
    const path = join(rootDir, file);
    filesRead.push(path);
    return readFileSync(path, "utf8");
  };

  const eslintFile = [".eslintrc.json", ".eslintrc"].find((file) => entries.has(file));
  if (eslintFile) {
    const json = tryParseJson(readFile(eslintFile));
    if (json) {
      const extracted = eslintRules(json, eslintFile);
      if (extracted.length > 0) {
        rules.push(...extracted);
        sources.add(eslintFile);
      }
    }
  }

  if (entries.has("biome.json")) {
    const json = tryParseJson(readFile("biome.json"));
    if (json) {
      const extracted = biomeRules(json, "biome.json");
      if (extracted.length > 0) {
        rules.push(...extracted);
        sources.add("biome.json");
      }
    }
  }

  const prettierFile = [".prettierrc", ".prettierrc.json"].find((file) => entries.has(file));
  if (prettierFile) {
    const json = tryParseJson(readFile(prettierFile));
    if (json) {
      const extracted = prettierRules(json, prettierFile);
      if (extracted.length > 0) {
        rules.push(...extracted);
        sources.add(prettierFile);
      }
    }
  }

  if (entries.has(".editorconfig")) {
    const extracted = parseEditorConfig(readFile(".editorconfig"), ".editorconfig");
    if (extracted.length > 0) {
      rules.push(...extracted);
      sources.add(".editorconfig");
    }
  }

  if (entries.has("rustfmt.toml")) {
    const extracted = parseFlatToml(readFile("rustfmt.toml"), "rustfmt.toml", "rustfmt");
    if (extracted.length > 0) {
      rules.push(...extracted);
      sources.add("rustfmt.toml");
    }
  }

  if (entries.has("detekt.yml")) {
    filesRead.push(join(rootDir, "detekt.yml"));
    rules.push({
      id: "detekt:present",
      description:
        "detekt configuration present (contents not parsed — see repository-intelligence risks)",
      source: { file: "detekt.yml", detail: "detekt.yml present" },
    });
    sources.add("detekt.yml");
  }

  for (const file of RULE_SHAPED_DOCS) {
    if (entries.has(file)) {
      const extracted = extractHeadingRules(readFile(file), file);
      if (extracted.length > 0) {
        rules.push(...extracted);
        sources.add(file);
      }
    }
  }

  const profile: CriteriaProfile = { rules, sources: [...sources].sort() };
  return { profile, filesRead, dirsListed: [] };
}
