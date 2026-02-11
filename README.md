# DEBUGGATHA

[![VS Code Version](https://img.shields.io/badge/VS%20Code-1.80%2B-blue.svg)](https://code.visualstudio.com/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.2-3178c6.svg)](https://www.typescriptlang.org/)

**AI-Powered Code Auditor for Visual Studio Code**

Debuggatha is a static analysis and code review tool that uses large language models to identify bad practices, antipatterns, and technical debt in your workspace. Think of it as an opinionated linter with personality—choose between harsh critique, gentle mentorship, or architectural wisdom.

---

## What It Does

Debuggatha performs three types of analysis on your code:

### 🔍 Generate Report
**Fast, high-level scan of selected files.**

- Identifies obvious issues (unused imports, magic numbers, naming violations)
- Flags common antipatterns (god objects, deep nesting, hardcoded secrets)
- Lists technical debt at a glance
- No deep architectural analysis—just the low-hanging fruit

**Use when:** You want a quick health check before commit/PR.

### 🛡️ Generate Audit
**Comprehensive security and quality audit.**

- Security vulnerabilities (injection risks, insecure dependencies, auth flaws)
- Performance bottlenecks (O(n²) algorithms, unnecessary re-renders, memory leaks)
- Code smells (tight coupling, lack of error handling, mutable global state)
- SOLID principle violations
- Accessibility issues (in UI code)

**Use when:** Preparing for production, reviewing legacy code, or conducting formal code review.

### 🧠 Deep Analysis
**Architectural critique and refactoring guidance.**

- Design pattern violations
- Abstraction leaks and tight coupling
- Scalability concerns
- Testability issues
- Refactoring roadmap with prioritized changes

**Use when:** Planning a refactor, evaluating architectural decisions, or mentoring junior developers.

---

## Personality System

Debuggatha delivers analysis in three distinct tones:

| Personality | Style | Best For |
|-------------|-------|----------|
| **Angry Witch** | Harsh, sarcastic, technically correct. Mocks bad code while providing solutions. | Experienced developers who want unfiltered truth. |
| **Kind Witch** | Empathetic, supportive, educational. Explains *why* something is wrong. | Beginners, stressful deadlines, or learning new tech. |
| **Wise Witch** | Professional, architectural, strategic. Focuses on long-term maintainability. | Senior reviews, design discussions, refactoring plans. |

Select personality from the dropdown in the Debuggatha sidebar. It applies to all actions (Report/Audit/Analysis).

---

## Multi-Model Support

Debuggatha works with **four AI providers** via native `fetch()` calls (no vendor SDKs):

- **Google Gemini** (gemini-1.5-pro, gemini-1.5-flash)
- **Anthropic Claude** (claude-3-5-sonnet-20241022, claude-3-5-haiku-20241022)
- **OpenAI GPT** (gpt-4o, gpt-4o-mini, gpt-4-turbo)
- **xAI Grok** (grok-beta)

Configure via:
1. **Command Palette**: `Debuggatha: Configure API Key` → Select provider → Enter key
2. **Settings UI**: Search for `debuggatha` → Set provider/model
3. **settings.json**: 
   ```json
   {
     "debuggatha.provider": "claude",
     "debuggatha.model": "claude-3-5-sonnet-20241022"
   }
   ```

API keys are stored in VS Code SecretStorage (encrypted, never in plaintext).

---

## Installation

### From Source
```bash
git clone https://github.com/HoujouSxnnyside/Debuggatha.git
cd Debuggatha
npm install
npm run compile
code --install-extension .
```

### From VSIX (future)
```bash
code --install-extension debuggatha-2.0.0.vsix
```

---

## Usage

### Quick Start

1. Open Debuggatha sidebar (witch icon in activity bar)
2. Configure API key: `Cmd/Ctrl+Shift+P` → `Debuggatha: Configure API Key`
3. Select personality (Angry/Kind/Wise)
4. Select provider and model from dropdowns
5. Select files from your workspace (max 10 files, 100KB each by default)
6. Click **Generate Report**, **Generate Audit**, or **Deep Analysis**

### File Selection

Debuggatha only reads files from your current workspace—**no OS file picker dialogs**.

- Click "Select Files from Workspace"
- Multi-select with `Cmd/Ctrl` (up to `debuggatha.maxFilesPerRequest`)
- Files are displayed in the sidebar with remove buttons
- Only workspace files are shown (respects `.gitignore`)

This prevents accidental leakage of files outside your project (e.g., system files, sensitive configs).

### Follow-Up Questions

After receiving analysis, use the input field to ask follow-up questions:

- "Show me how to refactor the `UserService` class"
- "Explain the O(n²) issue in more detail"
- "What's the security risk with line 45?"

Follow-ups maintain context from the original analysis.

---

## Configuration

### Extension Settings

- `debuggatha.provider` - AI provider (`gemini`, `claude`, `openai`, `grok`)
- `debuggatha.model` - Model to use (provider-specific, see dropdown)
- `debuggatha.defaultPersonality` - Default personality (`angry`, `kind`, `wise`)
- `debuggatha.maxFileSize` - Max file size in bytes (default: 102400 = 100KB)
- `debuggatha.maxFilesPerRequest` - Max files per action (default: 10)

### Commands

- `Debuggatha: Configure API Key` - Set/update API key for selected provider
- `Debuggatha: Clear Output` - Reset the analysis panel

---

## Security

### Data Handling

- **API keys**: Stored in VS Code SecretStorage (OS keychain/credential manager), never in plaintext
- **Code transmission**: Your code is sent to the selected AI provider's API. Review their privacy policies (Gemini, Claude, OpenAI, Grok).
- **No telemetry**: Debuggatha does not collect or transmit usage data to the extension author
- **Workspace-only**: File picker is restricted to workspace folders (cannot access system files)

### Vulnerability Reporting

**DO NOT open public issues for security vulnerabilities.**

Email: **security.sxnnyside@sxnnysideproject.com**

We will respond within 48 hours. See [SECURITY.md](SECURITY.md) for full policy.

---

## Support

- **Bug reports**: [GitHub Issues](https://github.com/HoujouSxnnyside/Debuggatha/issues)
- **Questions**: support.sxnnyside@sxnnysideproject.com
- **Security issues**: security.sxnnyside@sxnnysideproject.com

---

## Roadmap

- [ ] Export audit reports as Markdown/PDF
- [ ] Batch analysis (analyze entire workspace with one click)
- [ ] Custom rule engine (define project-specific antipatterns)
- [ ] Integration with ESLint/TSLint output
- [ ] Diff mode (analyze only changed files since last commit)
- [ ] Ollama support (local LLMs)

---

## Architecture

```
src/
  ├── extension.ts                # Entry point, command registration
  ├── types.ts                    # Shared type definitions
  ├── panels/
  │   └── DebuggathaPanel.ts      # Webview lifecycle, message router
  └── services/
      ├── LLMClientManager.ts     # Multi-provider abstraction (fetch-based)
      ├── PromptBuilder.ts        # Action-specific prompt engineering
      └── WorkspaceFileService.ts # File picker, reader

webview/
  ├── script.js                   # UI logic (action buttons, dropdowns)
  └── styles.css                  # VS Code-native styling (CSS variables)

media/
  ├── debuggatha-icon.svg         # Activity bar icon
  ├── angry-witch.svg             # Angry personality icon
  ├── kind-witch.svg              # Kind personality icon
  └── wise-witch.svg              # Wise personality icon

prompts/
  └── personalities.json          # Personality tone definitions
```

**Zero runtime dependencies.** All AI API calls use native `fetch()`.

---

## License

MIT License - See [LICENSE](LICENSE) for details.

---

## Credits

Built with TypeScript and the VS Code Extension API. No external AI SDKs.

For contribution guidelines, see [CONTRIBUTING.md](CONTRIBUTING.md).
