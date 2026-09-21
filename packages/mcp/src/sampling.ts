import type { SemanticProvider } from "@debuggatha/engine";
import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";

/**
 * The connected client's own model, asked through MCP sampling. The client decides whether
 * to answer (and usually asks its user first), so this sends code nowhere the person has not
 * already put it: to the assistant they are talking to.
 */
export function samplingProvider(server: McpServer): SemanticProvider {
  const provider: SemanticProvider = {
    name: "host",
    model: undefined,
    async complete({ system, user, maxTokens }) {
      const reply = await server.server.createMessage({
        systemPrompt: system,
        messages: [{ role: "user", content: { type: "text", text: user } }],
        maxTokens,
        temperature: 0,
      });
      provider.model = reply.model;
      if (reply.content.type !== "text")
        throw new Error("The client answered with something other than text.");
      return reply.content.text;
    },
  };
  return provider;
}

export function clientCanSample(server: McpServer): boolean {
  return Boolean(server.server.getClientCapabilities()?.sampling);
}
