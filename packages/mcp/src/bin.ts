import { parseArgs } from "node:util";
import { createInMemoryCache } from "@debuggatha/engine";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import pkg from "../package.json" with { type: "json" };
import { loadConfig } from "./config.js";
import { createDefaultDomainDeps } from "./domain-deps.js";
import { startHttpServer } from "./http.js";
import { createLogger } from "./logging.js";
import { createServer } from "./server.js";

const USAGE = `debuggatha-mcp ${pkg.version}

Usage: debuggatha-mcp [options]

Serves Debuggatha over MCP. Speaks stdio unless --http is given.

Options:
  --http           Serve Streamable HTTP at /mcp instead of stdio
  --host <host>    Interface for --http (default 127.0.0.1)
  --port <port>    Port for --http (default 3333)
  -v, --version    Print the version
  -h, --help       Print this help

Environment:
  DEBUGGATHA_REPOSITORY_ROOT   Repository reviewed when a call omits rootDir
  DEBUGGATHA_LOG_LEVEL         debug | info | warn | error (default info)
  DEBUGGATHA_CACHE_ENABLED     true | false (default true)
`;

async function run() {
  const { values } = parseArgs({
    options: {
      http: { type: "boolean", default: false },
      host: { type: "string", default: "127.0.0.1" },
      port: { type: "string", default: "3333" },
      version: { type: "boolean", short: "v", default: false },
      help: { type: "boolean", short: "h", default: false },
    },
  });

  if (values.help) return void process.stdout.write(USAGE);
  if (values.version) return void process.stdout.write(`${pkg.version}\n`);

  const config = loadConfig(process.env);
  const logger = createLogger({ minLevel: config.logLevel });
  const deps = createDefaultDomainDeps();
  const cache = config.cache.enabled ? createInMemoryCache() : undefined;
  const build = () => createServer(deps, config, logger, cache);

  logger.info("server.starting", {
    transport: values.http ? "http" : "stdio",
    defaultRepositoryRoot: config.defaultRepositoryRoot,
    cacheEnabled: config.cache.enabled,
    logLevel: config.logLevel,
  });

  if (values.http) {
    const port = Number(values.port);
    if (!Number.isInteger(port) || port < 0 || port > 65535) {
      throw new Error(`Invalid --port "${values.port}".`);
    }
    const httpServer = await startHttpServer({
      host: values.host,
      port,
      logger,
      createServer: build,
    });
    logger.info("server.running", { url: `http://${values.host}:${port}/mcp` });
    const stop = () => httpServer.close(() => process.exit(0));
    process.on("SIGINT", stop);
    process.on("SIGTERM", stop);
    return;
  }

  const server = build();
  const stop = async (signal: string) => {
    logger.info("server.shutdown.signal", { signal });
    await server.close();
    process.exit(0);
  };
  process.on("SIGINT", () => void stop("SIGINT"));
  process.on("SIGTERM", () => void stop("SIGTERM"));

  await server.connect(new StdioServerTransport());
  logger.info("server.running");
}

run().catch((error) => {
  process.stderr.write(
    `FATAL: Failed to start MCP server: ${error instanceof Error ? error.stack : String(error)}\n`,
  );
  process.exit(1);
});
