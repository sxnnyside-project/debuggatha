import type { Capability } from "./types.js";

const KIND_LABELS: Record<Capability["kind"], string> = {
  language: "Languages",
  framework: "Frameworks",
  platform: "Platforms",
  tooling: "Tooling",
  characteristic: "Characteristics",
};

const DISPLAY_OVERRIDES: Record<string, string> = {
  typescript: "TypeScript",
  javascript: "JavaScript",
  nodejs: "Node.js",
  nextjs: "Next.js",
  aspnetcore: "ASP.NET Core",
  vscodeextension: "VS Code Extension",
  "vscode-extension": "VS Code Extension",
};

function displayName(id: string): string {
  if (DISPLAY_OVERRIDES[id]) return DISPLAY_OVERRIDES[id] as string;
  return id.charAt(0).toUpperCase() + id.slice(1);
}

/**
 * Repository Summary (Epic 12.5): human-readable output *generated from*
 * the capability model, never the other way around — "Display labels
 * are outputs. Capabilities are the source of truth." Deterministic:
 * grouped by kind, each group's ids sorted, so the same `Capability[]`
 * always produces the same string.
 */
export function summarizeCapabilities(capabilities: readonly Capability[]): string {
  if (capabilities.length === 0) return "No capabilities detected.";

  const byKind = new Map<Capability["kind"], string[]>();
  for (const cap of capabilities) {
    const list = byKind.get(cap.kind) ?? [];
    list.push(displayName(cap.id));
    byKind.set(cap.kind, list);
  }

  const order: Capability["kind"][] = [
    "language",
    "framework",
    "platform",
    "tooling",
    "characteristic",
  ];
  const parts: string[] = [];
  for (const kind of order) {
    const names = byKind.get(kind);
    if (!names || names.length === 0) continue;
    parts.push(`${KIND_LABELS[kind]}: ${[...new Set(names)].sort().join(", ")}`);
  }

  return parts.join(" — ");
}
