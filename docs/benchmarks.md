# Benchmarks

Baseline performance measurements for Debuggatha CLI.

## Measurement Protocol

Run with:
```bash
just bench
```

Or measure end-to-end wall time using `hyperfine`:
```bash
hyperfine --warmup 3 'bun packages/cli/src/bin.ts --version'
hyperfine --warmup 1 'bun packages/cli/src/bin.ts review --files packages/core/src/index.ts --quiet'
```

## Baselines

| Date | Environment | Command | Median Runtime | Notes |
|---|---|---|---|---|
| 2026-11-10 | macOS (arm64, Apple Silicon) | `debuggatha --version` | ~25ms | Startup overhead |
| 2026-11-10 | macOS (arm64, Apple Silicon) | `debuggatha review --files packages/core/src/index.ts --quiet` | ~180ms | Deterministic core review |


## Rust engine against the TypeScript engine

2026-11-18, macOS arm64 (Apple silicon), release build of `debuggatha` (LTO, stripped) against `bun packages/cli/src/bin.ts`, run from the repository root (408 tracked files, 205 of them Rust). `hyperfine`, 2 warmups, 8 runs, `--fail-on none --quiet --no-persist --no-analyzers`; peak memory from `/usr/bin/time -l`.

| Command | Rust | TypeScript |
|---|---|---|
| `--version` | 4 ms | 46 ms (Bun), 102 ms (Node, bundled) |
| `review --files packages/core/src/index.ts` | 44 ms, 12 MB | 55 ms, 43 MB |
| `review --all` | 1.58 s, 88 MB | 0.89 s, 119 MB |

- Startup and a single file favor Rust. The whole repository does not: Rust is about 1.8 times slower. Part of that is not like for like: the Rust engine parses and reviews the Rust files with tree-sitter and the TypeScript engine has no Rust syntax tree, so it does less work on this repository. The numbers were not taken on a corpus of one language, so the gap is not attributed.
- Before the fixes below the same run took 3.3 s (whole repository) and 148 ms (one file). Profiling found three causes, all fixed: the queries of every language were compiled on the first use of any (now each language compiles when a file of it appears); the call-to-concept lookup compared every call against every wildcard entry (now exact entries are a map and patterns are rejected by their prefix and suffix first); and the environment, pattern and query detectors compiled all their languages eagerly.
- With analyzers on, whatever is installed runs in both engines (Biome here adds about 0.4 s to each); that is the tool's time.
- Not measured: Linux and Windows; a repository of a single language; the semantic pass; a cold disk cache; a repository larger than this one. The remaining time is spread over parsing, masking and query matching with no single hot spot.
