import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

export default defineConfig({
  resolve: {
    alias: { vscode: fileURLToPath(new URL("./src/test/vscode-mock.ts", import.meta.url)) },
  },
  test: {
    include: ["src/**/*.test.ts"],
    exclude: ["src/test/integration/**"],
  },
});
