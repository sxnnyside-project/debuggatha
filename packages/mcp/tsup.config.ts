import { defineConfig } from "tsup";

// Self-contained bundle: every @debuggatha/* workspace package is inlined so the
// published tarball depends only on the third-party packages in `dependencies`.
export default defineConfig({
  entry: ["src/bin.ts"],
  format: ["esm"],
  target: "node20",
  platform: "node",
  clean: true,
  sourcemap: true,
  noExternal: [/^@debuggatha\//],
  banner: {
    js: "#!/usr/bin/env node",
  },
});
