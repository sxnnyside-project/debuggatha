import { defineConfig } from "tsup";

export default defineConfig({
  entry: ["src/index.ts"],
  format: ["cjs"],
  target: "es2022",
  clean: true,
  dts: true,
  sourcemap: true,
  external: ["vscode"],
});
