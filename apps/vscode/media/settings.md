# Configuring Debuggatha

Debuggatha is designed to be zero-configuration for standard projects. It uses the `Foundation Bundle` to automatically detect your tech stack (e.g., React, Go, Kotlin) and apply the correct semantic rules.

However, you can configure its behavior directly through VS Code Settings.

## Available Settings

- **Review Depth**: Choose between Quick, Full, or Architectural reviews depending on how deep you want the context window to go.
- **Default Review Packs**: Manually define fallback Review Packs if auto-detection fails.
- **Runtime**: Run the review engine completely locally (Local) or connect to an enterprise MCP server (MCP).
