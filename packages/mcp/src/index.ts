import { createInMemoryCache } from "@debuggatha/core";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { loadConfig } from "./config.js";
import { createDefaultDomainDeps } from "./domain-deps.js";
import { createLogger } from "./logging.js";
import { createServer } from "./server.js";

export { loadConfig } from "./config.js";
export { createDefaultDomainDeps } from "./domain-deps.js";
export { createLogger } from "./logging.js";
export { createServer } from "./server.js";

async function run() {
  const config = loadConfig(process.env);
  const logger = createLogger({ minLevel: config.logLevel });

  logger.info("server.starting", {
    defaultRepositoryRoot: config.defaultRepositoryRoot,
    cacheEnabled: config.cache.enabled,
    logLevel: config.logLevel,
  });

  const deps = createDefaultDomainDeps();
  const cache = config.cache.enabled ? createInMemoryCache() : undefined;

  const server = createServer(deps, config, logger, cache);
  const transport = new StdioServerTransport();

  // Handle cleanup on exit/shutdown
  process.on("SIGINT", async () => {
    logger.info("server.shutdown.signal", { signal: "SIGINT" });
    await server.close();
    process.exit(0);
  });

  process.on("SIGTERM", async () => {
    logger.info("server.shutdown.signal", { signal: "SIGTERM" });
    await server.close();
    process.exit(0);
  });

  await server.connect(transport);
  logger.info("server.running");
}

// Execution check
const isMain =
  typeof process !== "undefined" &&
  process.argv[1] &&
  (process.argv[1].endsWith("dist/index.js") ||
    process.argv[1].endsWith("src/index.ts") ||
    process.argv[1].endsWith("debuggatha-mcp"));

if (isMain && !process.env.VITEST) {
  run().catch((error) => {
    process.stderr.write(
      `FATAL: Failed to start MCP server: ${error instanceof Error ? error.stack : String(error)}\n`,
    );
    process.exit(1);
  });
}
