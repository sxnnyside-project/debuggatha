import type { ReviewPack } from "@debuggatha/knowledge-system";

export const electronPack: ReviewPack = {
  id: "debuggatha/electron",
  version: "1.0.0",
  kind: "stack",
  displayName: "Electron",
  dependsOn: [],
  rules: [
    {
      id: "node-integration-false",
      packId: "debuggatha/electron",
      statement:
        "Always set `nodeIntegration: false` in `webPreferences` when creating a BrowserWindow.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "electron" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-electron-node-integration"],
      contradicts: undefined,
    },
    {
      id: "context-isolation-true",
      packId: "debuggatha/electron",
      statement:
        "Always set `contextIsolation: true` in `webPreferences`. This is mandatory for securing IPC.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "electron" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-electron-context-isolation"],
      contradicts: undefined,
    },
    {
      id: "disable-remote-module",
      packId: "debuggatha/electron",
      statement:
        "Do not use the `@electron/remote` module. It severely compromises the boundary between the main and renderer processes.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "electron" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-electron-remote"],
      contradicts: undefined,
    },
    {
      id: "ipc-validation",
      packId: "debuggatha/electron",
      statement:
        "Validate all arguments received via `ipcMain.on` and `ipcMain.handle`. The renderer process is untrusted.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "electron" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-electron-ipc"],
      contradicts: undefined,
    },
    {
      id: "sandbox-true",
      packId: "debuggatha/electron",
      statement: "Enable Chromium's sandbox via `sandbox: true` for all BrowserWindows.",
      category: "security",
      appliesTo: { kind: "requires-framework", framework: "electron" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-electron-sandbox"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-electron-node-integration",
      title: "Node Integration",
      body: "If `nodeIntegration` is enabled, the renderer process has full access to Node.js APIs (e.g., `fs`, `child_process`). If the renderer suffers an XSS attack, the attacker gains immediate RCE (Remote Code Execution) on the victim's machine.",
      externalRefs: [
        "https://www.electronjs.org/docs/latest/tutorial/security#2-do-not-enable-nodejs-integration-for-remote-content",
      ],
    },
    {
      id: "know-electron-context-isolation",
      title: "Context Isolation",
      body: "Without `contextIsolation`, the renderer's JavaScript environment (window) is shared with the preload script. An attacker can use Prototype Pollution to intercept or alter the behavior of native APIs exposed by the preload script.",
      externalRefs: ["https://www.electronjs.org/docs/latest/tutorial/context-isolation"],
    },
    {
      id: "know-electron-remote",
      title: "The Remote Module is Dangerous",
      body: "The remote module allows the renderer process to invoke methods on main process objects directly. This creates a massive attack surface and causes severe performance bottlenecks due to synchronous IPC blocking.",
      externalRefs: ["https://nvd.nist.gov/vuln/detail/CVE-2022-29247"],
    },
    {
      id: "know-electron-ipc",
      title: "Untrusted Renderer Process",
      body: "The renderer process must be treated exactly like a remote client hitting a web server. If `ipcMain.handle('delete-file', (e, path) => fs.unlinkSync(path))` is implemented without validation, any script in the renderer can delete arbitrary files.",
      externalRefs: ["https://www.electronjs.org/docs/latest/tutorial/ipc"],
    },
    {
      id: "know-electron-sandbox",
      title: "Chromium Sandbox",
      body: "The sandbox limits the actions the renderer process can perform at the OS level, isolating it from the filesystem and restricting its capabilities even if the V8 engine is completely compromised.",
      externalRefs: ["https://www.electronjs.org/docs/latest/tutorial/sandbox"],
    },
  ],
};
