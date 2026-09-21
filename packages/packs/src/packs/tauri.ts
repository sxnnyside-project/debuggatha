import type { ReviewPack } from "@debuggatha/core";

export const tauriPack: ReviewPack = {
  id: "debuggatha/tauri",
  version: "1.0.0",
  kind: "stack",
  displayName: "Tauri",
  dependsOn: [],
  rules: [
    {
      id: "ipc-input-validation",
      packId: "debuggatha/tauri",
      statement:
        "Always validate and sanitize data received via Tauri IPC (`invoke`) in the Rust backend. Never trust client payloads.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "tauri" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-tauri-ipc"],
      contradicts: undefined,
    },
    {
      id: "allowlist-minimization",
      packId: "debuggatha/tauri",
      statement:
        "Minimize the Tauri `allowlist` in `tauri.conf.json`. Only enable the specific API endpoints (e.g., `fs`, `shell`) required by the application.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "tauri" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-tauri-allowlist"],
      contradicts: undefined,
    },
    {
      id: "csp-configuration",
      packId: "debuggatha/tauri",
      statement:
        "Define a strict Content Security Policy (CSP) in `tauri.conf.json` preventing external script execution and inline scripts.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "tauri" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-tauri-csp"],
      contradicts: undefined,
    },
    {
      id: "avoid-shell-execute",
      packId: "debuggatha/tauri",
      statement:
        "Avoid using the `shell.execute` or `shell.open` APIs with unvalidated user input to prevent command injection.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "tauri" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-tauri-shell"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-tauri-ipc",
      title: "Tauri Inter-Process Communication",
      body: "The frontend is a webview that can be manipulated by malicious scripts (if XSS occurs). The Rust backend must treat all IPC calls identically to public HTTP API endpoints, assuming all parameters are potentially hostile.",
      externalRefs: ["https://tauri.app/v1/guides/architecture/inter-process-communication/"],
      limitations: [
        "A command handler that only receives primitive/enum arguments with no filesystem, shell, or resource-identifier semantics (e.g. a pure math or string-formatting command) has no meaningful validation to add, so flagging every `#[tauri::command]` without a validation call is a false positive for handlers with no sensitive input surface.",
      ],
    },
    {
      id: "know-tauri-allowlist",
      title: "Principle of Least Privilege",
      body: "The allowlist controls which native APIs the frontend can access. Enabling 'all' provides any XSS vulnerability with total access to the user's filesystem and shell, elevating a Web vulnerability to a system compromise.",
      externalRefs: ["https://tauri.app/v1/api/config/#allowlistconfig"],
      limitations: [
        "Tauri v2 replaced the global `tauri.conf.json` allowlist with per-plugin, per-command capability/permission files (`src-tauri/capabilities/*.json`); a rule that only greps `tauri.conf.json` for `\"all\": true` will silently miss an overly broad v2 capability grant, and will false-positive as 'no allowlist found' on a v2 project that migrated away from that key entirely.",
      ],
    },
    {
      id: "know-tauri-csp",
      title: "Content Security Policy",
      body: "Tauri allows baking a CSP directly into the application configuration, preventing the webview from loading malicious external scripts even if an injection vulnerability exists.",
      externalRefs: ["https://tauri.app/v1/guides/security/#content-security-policy"],
      limitations: [
        "A dev build that intentionally sets `csp: null` to allow the Vite/webpack HMR dev server to inject scripts is expected and not itself a vulnerability — this rule should target the production CSP configuration, not flag every relaxed dev-mode setting as a violation.",
      ],
    },
    {
      id: "know-tauri-shell",
      title: "Command Injection",
      body: "Passing variables to shell commands directly can lead to OS command injection. If a user inputs `file.txt; rm -rf /`, the shell will execute the destructive command.",
      externalRefs: ["https://tauri.app/v1/api/js/shell/"],
      limitations: [
        'Using `Command.create`/`shell.open` with a fixed program name and an argument array (not a shell-interpolated string) is not vulnerable to shell metacharacter injection the way `sh -c "..."` string concatenation is — a rule that flags any use of the shell API regardless of whether arguments are passed as an array or interpolated into a string string will over-flag the safe array-argument form.',
      ],
    },
  ],
};
