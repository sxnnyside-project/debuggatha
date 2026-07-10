import type { ReviewPack } from "@debuggatha/knowledge-system";

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
    },
    {
      id: "know-tauri-allowlist",
      title: "Principle of Least Privilege",
      body: "The allowlist controls which native APIs the frontend can access. Enabling 'all' provides any XSS vulnerability with total access to the user's filesystem and shell, elevating a Web vulnerability to a system compromise.",
      externalRefs: ["https://tauri.app/v1/api/config/#allowlistconfig"],
    },
    {
      id: "know-tauri-csp",
      title: "Content Security Policy",
      body: "Tauri allows baking a CSP directly into the application configuration, preventing the webview from loading malicious external scripts even if an injection vulnerability exists.",
      externalRefs: ["https://tauri.app/v1/guides/security/#content-security-policy"],
    },
    {
      id: "know-tauri-shell",
      title: "Command Injection",
      body: "Passing variables to shell commands directly can lead to OS command injection. If a user inputs `file.txt; rm -rf /`, the shell will execute the destructive command.",
      externalRefs: ["https://tauri.app/v1/api/js/shell/"],
    },
  ],
};
