import { createServer as createHttpServer, type IncomingMessage, type Server } from "node:http";
import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import type { Transport } from "@modelcontextprotocol/sdk/shared/transport.js";
import type { Logger } from "./logging.js";

export interface HttpOptions {
  host: string;
  port: number;
  logger: Logger;
  /** A fresh server per request keeps the endpoint stateless. */
  createServer: () => McpServer;
}

const LOOPBACK_HOSTS = new Set(["localhost", "127.0.0.1", "[::1]"]);

/** Blocks DNS-rebinding: a browser page cannot reach a loopback-bound server under a foreign Host. */
function hostIsAllowed(req: IncomingMessage, bindHost: string): boolean {
  const header = req.headers.host;
  if (!header) return false;
  const hostname = header.startsWith("[")
    ? header.slice(0, header.indexOf("]") + 1)
    : header.split(":")[0];
  return LOOPBACK_HOSTS.has(hostname ?? "") || hostname === bindHost;
}

async function readJson(req: IncomingMessage): Promise<unknown> {
  const chunks: Buffer[] = [];
  for await (const chunk of req) chunks.push(chunk as Buffer);
  return chunks.length === 0 ? undefined : JSON.parse(Buffer.concat(chunks).toString("utf8"));
}

export function startHttpServer(options: HttpOptions): Promise<Server> {
  const httpServer = createHttpServer(async (req, res) => {
    const reply = (status: number, message: string) => {
      res.writeHead(status, { "content-type": "application/json" });
      res.end(JSON.stringify({ jsonrpc: "2.0", error: { code: -32000, message }, id: null }));
    };

    if (!hostIsAllowed(req, options.host)) return reply(403, "Forbidden host.");
    if (new URL(req.url ?? "/", "http://localhost").pathname !== "/mcp") {
      return reply(404, "Not found. The MCP endpoint is /mcp.");
    }
    if (req.method !== "POST") {
      res.setHeader("allow", "POST");
      return reply(405, "Method not allowed. This endpoint is stateless: use POST.");
    }

    const server = options.createServer();
    // No session id generator = stateless mode.
    const transport = new StreamableHTTPServerTransport({});
    res.on("close", () => {
      void transport.close();
      void server.close();
    });
    try {
      await server.connect(transport as Transport);
      await transport.handleRequest(req, res, await readJson(req));
    } catch (error) {
      options.logger.error("http.request.failure", {
        message: error instanceof Error ? error.message : String(error),
      });
      if (!res.headersSent) reply(400, "Invalid request.");
    }
  });

  return new Promise((resolve, reject) => {
    httpServer.once("error", reject);
    httpServer.listen(options.port, options.host, () => resolve(httpServer));
  });
}
