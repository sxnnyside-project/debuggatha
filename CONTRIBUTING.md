# Contributing to Debuggatha

We welcome contributions from the community. Whether you're fixing a bug, adding a feature, or improving documentation, your help makes Debuggatha better.

---

## Before You Start

### Code of Conduct

Be professional. Be respectful. Be constructive. We're all here to build better tools.

### Communication Channels

- **Bug reports / Feature requests**: [GitHub Issues](https://github.com/HoujouSxnnyside/Debuggatha/issues)
- **General questions**: support.sxnnyside@sxnnysideproject.com
- **Security issues**: security.sxnnyside@sxnnysideproject.com (see [SECURITY.md](SECURITY.md))

---

## Development Setup

### Prerequisites

- Node.js 18.x or higher
- VS Code 1.80.0 or higher
- Git
- TypeScript knowledge

### Local Setup

```bash
# Clone the repo
git clone https://github.com/HoujouSxnnyside/Debuggatha.git
cd Debuggatha

# Install dependencies
npm install

# Compile TypeScript
npm run compile

# Launch Extension Development Host
# Press F5 in VS Code, or:
code --extensionDevelopmentPath=. --disable-extensions
```

### Project Structure

```
src/
  ├── extension.ts           # Entry point, command registration
  ├── types.ts               # Shared type definitions
  ├── panels/
  │   └── DebuggathaPanel.ts # Webview lifecycle, message router
  └── services/
      ├── LLMClientManager.ts       # Multi-provider AI abstraction
      ├── PromptBuilder.ts          # Prompt engineering per action type
      └── WorkspaceFileService.ts   # File picker, reader
webview/
  ├── script.js              # Webview UI logic
  └── styles.css             # VS Code–native styling
```

---

## How to Contribute

### Reporting Bugs

Before opening an issue, search existing issues to avoid duplicates.

**Include:**
- Clear, descriptive title
- Steps to reproduce
- Expected vs. actual behavior
- VS Code version (`Help > About`)
- Debuggatha version (`Extensions` panel)
- Relevant log output (open Developer Console in webview)

**Example:**
```
Title: "Generate Audit" fails with 429 error on Gemini 1.5 Pro

Steps:
1. Configure Gemini API key
2. Select 5 TypeScript files
3. Click "Generate Audit"
4. Error: "Rate limit exceeded"

Expected: Audit completes successfully
Actual: 429 error immediately

Environment:
- VS Code 1.85.0
- Debuggatha 2.0.0
- macOS 14.2
- Gemini 1.5 Pro (via gemini-1.5-pro-latest)
```

### Suggesting Features

Feature requests should be:
- **Specific**: "Add ESLint integration" not "Make it better"
- **Justified**: Why is this useful? Who benefits?
- **Scoped**: Can it be implemented incrementally?

Use the `enhancement` label.

### Submitting Pull Requests

#### 1. Fork and Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/issue-123
```

#### 2. Code Standards

- **TypeScript**: Strict mode enabled. No `any` unless absolutely necessary.
- **Formatting**: Run `npm run lint` before committing.
- **Naming**: Use clear, descriptive names. Avoid abbreviations.
- **Comments**: Explain *why*, not *what*. Code should be self-documenting.

**Good:**
```typescript
// Rate limiting: Gemini free tier allows 1 req/sec
await this.sleep(1000);
```

**Bad:**
```typescript
// Wait 1 second
await this.sleep(1000);
```

#### 3. Commit Messages

Follow conventional commits:

```
feat(llm): add support for Mistral AI provider
fix(ui): prevent action buttons from being clickable while loading
docs(readme): clarify API key storage security model
refactor(prompt): extract personality guidelines into constants
```

**Format:**
```
<type>(<scope>): <subject>

<body (optional)>

<footer (optional)>
```

**Types:** `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`

#### 4. Testing

- Manually test your changes in Extension Development Host
- Ensure TypeScript compiles: `npm run compile`
- Check for lint errors: `npm run lint`

We don't have automated tests yet (contributions welcome!), but manual testing is required:
- Test all three personalities
- Test all three actions (Report / Audit / Analysis)
- Test with multiple AI providers if relevant
- Test file selection with 0, 1, and 10+ files

#### 5. Documentation

Update relevant docs:
- If adding a feature: Update [README.md](README.md)
- If changing configuration: Update settings schema in [package.json](package.json)
- If adding a new service: Add architectural notes

#### 6. Submit PR

**Title:** Same format as commit messages  
**Description:** Include:
- What does this change?
- Why is it needed?
- How was it tested?
- Are there breaking changes?

Link to related issues: `Closes #123`

---

## Architectural Guidelines

### No Runtime Dependencies

Debuggatha has **zero** runtime dependencies (no AI SDKs). All API calls use native `fetch()`. This keeps bundle size minimal and avoids supply chain risks.

**Don't add dependencies unless:**
- Absolutely necessary
- Well-maintained (updated in last 6 months)
- Small bundle size (<50KB)
- No transitive dependency explosion

### VS Code Native Patterns

- Use VS Code APIs directly (don't abstract unnecessarily)
- Follow VS Code UX patterns (no custom dialogs when QuickPick exists)
- Use VS Code's theming system (CSS variables only)
- Store secrets in `SecretStorage`, never in settings

### Prompt Engineering

All prompts are constructed in `PromptBuilder.ts`:
- Use structured text, not JSON
- Inject personality at the system level
- Keep prompts deterministic (no randomness in construction)
- Test with all three personalities

### Error Handling

- All API calls must have try/catch
- User-facing errors must be actionable ("Click X to configure API key", not "Error 401")
- Log technical details to console, show simple messages to users

---

## Review Process

1. **Automated checks** (if configured): TypeScript compilation, linting
2. **Maintainer review**: Code quality, architecture fit, security implications
3. **Testing**: Manual testing in different environments
4. **Merge**: Squash commits into a single, well-formatted commit

---

## Recognition

Contributors will be:
- Listed in release notes
- Credited in GitHub commit history
- Appreciated by the maintainers

Significant contributions may result in:
- Maintainer status (if interested)
- Design decision input

---

## Questions?

If you're unsure about anything:
- Open a draft PR and ask for early feedback
- Email support.sxnnyside@sxnnysideproject.com
- Open a discussion issue (use `question` label)

We'd rather answer questions early than review a large PR that doesn't fit the project direction.

---

**Thank you for contributing to Debuggatha!**
