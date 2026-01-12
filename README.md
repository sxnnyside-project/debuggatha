# 🧙‍♀️ Debuggatha

**Debuggatha** is an AI-powered debugging assistant for Visual Studio Code that makes debugging engaging and genuinely useful through personality-driven interactions.

## Features

- **🎭 Three Distinct Witch Personalities**
  - **Angry Witch**: Harsh but technically correct. Mocks bad practices while providing solutions
  - **Kind Witch**: Empathetic and supportive. Perfect for beginners or stressful sessions
  - **Wise Witch**: Professional and architectural. Focuses on best practices and design

- **💬 Interactive Chat Interface**
  - Seamless sidebar chat panel
  - Real-time AI responses powered by Google Gemini
  - Beautiful, VS Code-themed UI

- **📁 Context-Aware Analysis**
  - Select multiple project files for context
  - Smart file reading with size limits
  - Language-aware code analysis

- **🔐 Secure & Production-Ready**
  - API keys stored in VS Code SecretStorage
  - Input sanitization and validation
  - Rate limiting and error handling
  - Network resilience

## Installation

### Prerequisites

- Visual Studio Code 1.74.0 or higher
- Node.js 18.x or higher
- A [Google Gemini API Key](https://makersuite.google.com/app/apikey)

### Setup

1. Clone or download this repository
2. Open in VS Code
3. Install dependencies:
   \`\`\`bash
   npm install
   \`\`\`
4. Compile the extension:
   \`\`\`bash
   npm run compile
   \`\`\`
5. Press \`F5\` to launch the Extension Development Host

### First Time Configuration

1. Open the Debuggatha sidebar (witch icon in activity bar)
2. Run command: \`Debuggatha: Set Gemini API Key\`
3. Enter your Gemini API key (it will be stored securely)
4. Start debugging with your chosen witch!

## Usage

### Basic Workflow

1. **Select a Personality**: Click one of the three witch icons at the top
2. **Add Context** (optional): Click "📁 Add Files to Context" to select relevant files
3. **Ask Your Question**: Type your debugging question or describe the issue
4. **Invoke the Witch**: Click "✨ Invoke Witch" or press Enter

### Example Questions

- "Why is my async function not awaiting properly?"
- "This component is re-rendering too much, help me optimize it"
- "I'm getting a null pointer exception on line 45, what's wrong?"
- "Review this code for security issues"
- "Explain why this algorithm is O(n²) and suggest improvements"

### Personality Examples

**Angry Witch Response:**
> Seriously? You're not awaiting your promises? This is basic async/await 101! 🤦‍♀️ Here's what you SHOULD have done...

**Kind Witch Response:**
> Don't worry! Async/await can be tricky at first. Let me walk you through this gently. The issue here is...

**Wise Witch Response:**
> The architectural concern here involves the event loop and microtask queue. Let's examine the proper async pattern for optimal performance...

## Architecture

### Project Structure

\`\`\`
debuggatha/
├── src/
│   ├── extension.ts              # Entry point, activation logic
│   ├── panels/
│   │   └── DebuggathaPanel.ts    # Webview lifecycle & messaging
│   └── services/
│       ├── GeminiClient.ts       # AI API integration
│       └── PromptBuilder.ts      # Dynamic prompt construction
├── webview/
│   ├── script.js                 # Webview UI logic
│   └── styles.css                # UI styling
├── media/
│   ├── angry-witch.svg           # Angry personality icon
│   ├── kind-witch.svg            # Kind personality icon
│   ├── wise-witch.svg            # Wise personality icon
│   └── witch-icon.svg            # Activity bar icon
├── prompts/
│   └── personalities.json        # Personality configurations
└── package.json                  # Extension manifest
\`\`\`

### Key Components

#### GeminiClient (\`services/GeminiClient.ts\`)
- Manages Gemini API connections
- Handles rate limiting (1 request/second minimum)
- Comprehensive error handling (rate limits, safety filters, network issues)
- Secure API key retrieval from VS Code SecretStorage

#### PromptBuilder (\`services/PromptBuilder.ts\`)
- Constructs optimized prompts with personality injection
- Includes system role, tone guidelines, and behavioral rules
- Formats file context with language detection
- Input sanitization and length limits

#### DebuggathaPanel (\`panels/DebuggathaPanel.ts\`)
- Webview lifecycle management
- Bidirectional messaging between extension and webview
- Chat history persistence
- File picker integration

## Configuration

### Extension Settings

- \`debuggatha.defaultPersonality\`: Default witch personality (\`angry\`, \`kind\`, or \`wise\`)
- \`debuggatha.maxFileSize\`: Maximum file size to send to AI (default: 100KB)
- \`debuggatha.maxFilesPerRequest\`: Maximum files per request (default: 5)

### Commands

- \`Debuggatha: Open Chat\` - Open the chat panel
- \`Debuggatha: Set Gemini API Key\` - Configure your API key
- \`Debuggatha: Clear Chat History\` - Reset the conversation

## Development

### Building

\`\`\`bash
# Compile TypeScript
npm run compile

# Watch mode (auto-recompile on changes)
npm run watch

# Lint code
npm run lint
\`\`\`

### Testing

Press \`F5\` in VS Code to launch the Extension Development Host with your changes.

### Debugging

- Extension logs are visible in the Debug Console
- Webview console: Right-click in the Debuggatha panel → "Open Webview Developer Tools"

## Security & Privacy

- **API Keys**: Stored in VS Code SecretStorage, never in plaintext
- **Input Validation**: All user input is sanitized before sending to AI
- **File Size Limits**: Prevents sending excessive data to external APIs
- **No Data Retention**: Conversations are stored locally only, not sent to external servers except the AI API
- **Content Safety**: Gemini's built-in safety filters prevent harmful content

## Troubleshooting

### "Gemini API key not found"
Run \`Debuggatha: Set Gemini API Key\` command and enter your key.

### "Rate limit exceeded"
Wait a moment before making the next request. Consider upgrading your Gemini API plan.

### "File too large" messages
Reduce the number or size of files you're including, or adjust \`debuggatha.maxFileSize\` setting.

### Extension not activating
Ensure you're using VS Code 1.74.0 or higher and have compiled the TypeScript (\`npm run compile\`).

## Extensibility

### Adding Custom Personalities

Edit \`prompts/personalities.json\`:

\`\`\`json
{
  "personalities": {
    "yourpersonality": {
      "system": "Your system role description",
      "tone": "Your tone description",
      "guidelines": [
        "Behavioral guideline 1",
        "Behavioral guideline 2"
      ]
    }
  }
}
\`\`\`

Update the type definitions and UI accordingly.

### Changing AI Provider

To switch from Gemini to another provider:

1. Modify \`GeminiClient.ts\` to use different API
2. Update authentication mechanism
3. Adjust prompt format if needed
4. Update package.json dependencies

## License

MIT License - Feel free to use and modify for your needs.

## Credits

Created as a production-ready VS Code extension demonstrating:
- Advanced webview architecture
- LLM integration best practices
- Personality-driven AI interactions
- Secure API key management
- Professional TypeScript patterns

---

**Made with 🧙‍♀️ magic and serious engineering**
