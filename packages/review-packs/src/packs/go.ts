import type { ReviewPack } from "@debuggatha/knowledge-system";

export const goPack: ReviewPack = {
  id: "debuggatha/go",
  version: "1.0.0",
  kind: "stack",
  displayName: "Go",
  dependsOn: [],
  rules: [
    {
      id: "explicit-error-check",
      packId: "debuggatha/go",
      statement:
        "Always check returned errors using `if err != nil`. Never assign an error to the blank identifier `_`.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-go-error-checking"],
      contradicts: undefined,
    },
    {
      id: "prevent-goroutine-leaks",
      packId: "debuggatha/go",
      statement:
        "Ensure every goroutine has a guaranteed exit condition via context cancellation or closed channels.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-go-goroutine-leaks"],
      contradicts: undefined,
    },
    {
      id: "no-panic",
      packId: "debuggatha/go",
      statement:
        "Avoid `panic()`. Use it only for unrecoverable initialization errors (like missing config), never for control flow or request handling.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-go-panic"],
      contradicts: undefined,
    },
    {
      id: "defer-cleanup",
      packId: "debuggatha/go",
      statement:
        "Use `defer` immediately after opening a resource (files, locks, connections) to guarantee cleanup.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-go-defer"],
      contradicts: undefined,
    },
    {
      id: "prefer-interfaces",
      packId: "debuggatha/go",
      statement:
        "Define interfaces where they are consumed, not where they are implemented. Keep interfaces small (1-2 methods).",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-go-interfaces"],
      contradicts: undefined,
    },
    {
      id: "value-vs-pointer",
      packId: "debuggatha/go",
      statement:
        "Pass small structs by value to avoid heap escapes and GC pressure. Pass by pointer only when mutating the receiver or copying large structs.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-go-pointers"],
      contradicts: undefined,
    },
    {
      id: "no-global-state",
      packId: "debuggatha/go",
      statement:
        "Avoid mutable global variables (e.g., `var DB *sql.DB`). Pass dependencies explicitly via struct fields.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-go-global-state"],
      contradicts: undefined,
    },
    {
      id: "context-propagation",
      packId: "debuggatha/go",
      statement:
        "Always pass `context.Context` as the first argument to blocking operations or functions that span API boundaries.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "go" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-go-context"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-go-error-checking",
      title: "Explicit Error Handling",
      body: "Go relies on values to communicate errors. Ignoring an error value by assigning it to `_` forces the program into an undefined state where subsequent logic runs against invalid data, often leading to panics.",
      externalRefs: ["https://go.dev/doc/effective_go#errors"],
      limitations: [
        "Some stdlib calls return an error that is genuinely safe to ignore in context — e.g. `buf.WriteString` on a `bytes.Buffer` never actually errors — so a blanket 'every `_ = ` on an error-returning call is a violation' scan will false-positive on these well-known, documented no-op-error APIs.",
      ],
    },
    {
      id: "know-go-goroutine-leaks",
      title: "Goroutine Leaks",
      body: "A goroutine blocked on a channel that will never be written to or read from stays in memory forever, leaking its stack and variables. Always bind goroutine lifecycles to a `context.Context`.",
      externalRefs: ["https://go101.org/article/memory-leaking.html"],
      limitations: [
        "A goroutine launched in `main()` or in a long-lived worker pool that is intentionally meant to run for the entire process lifetime (e.g. a background metrics reporter) is not leaking just because it lacks an explicit `context.Context` — the rule's target is goroutines whose lifetime should be scoped to a shorter-lived request or operation but isn't.",
      ],
    },
    {
      id: "know-go-panic",
      title: "Panic vs Error",
      body: "Panics bypass the normal control flow and can crash the entire binary. They should strictly be reserved for 'impossible' invariants and fatal startup conditions, not for routing errors or malformed input.",
      externalRefs: ["https://go.dev/doc/effective_go#panic"],
      limitations: [
        "`MustCompile`-style helpers (e.g. `regexp.MustCompile`) and package-level `init()` panics on a hardcoded, developer-controlled value (not user input) are an accepted idiom for fail-fast startup validation — flagging every `panic()` call site uniformly, without distinguishing startup-time invariants from request-handling code, produces false positives on this common pattern.",
      ],
    },
    {
      id: "know-go-defer",
      title: "Resource Cleanup via Defer",
      body: "Calling `defer file.Close()` immediately after successful opening ensures that regardless of how many early returns or errors occur later in the function, the resource is deterministically freed.",
      externalRefs: ["https://go.dev/tour/flowcontrol/12"],
      limitations: [
        "In a hot loop, deferring cleanup inside each iteration accumulates all deferred calls until the enclosing function returns rather than running per-iteration, which can hold resources (file descriptors, locks) open far longer than intended — the fix there is an extracted per-iteration function, not necessarily adding `defer`, so flagging 'missing defer in a loop body' as the violation can point at the wrong fix.",
      ],
    },
    {
      id: "know-go-interfaces",
      title: "Consumer-Defined Interfaces",
      body: "Unlike Java or C#, Go interfaces are satisfied implicitly. This means packages implementing concrete types do not need to declare interfaces. Consumers should declare small, targeted interfaces (like `io.Reader`) specifically for the behavior they require.",
      externalRefs: ["https://go.dev/doc/effective_go#interfaces_and_types"],
      limitations: [
        "A producer-side interface is the correct, idiomatic choice when a package genuinely needs to expose multiple valid implementations to unknown future consumers (e.g. a plugin system or a driver registry pattern like `database/sql`) — flagging every interface declared alongside its implementation as a violation ignores this legitimate exception.",
      ],
    },
    {
      id: "know-go-pointers",
      title: "Escape Analysis and GC Pressure",
      body: "Passing pointers forces the compiler to allocate the variable on the heap, increasing Garbage Collection pressure. For small structs, passing by value is significantly faster because it stays on the stack.",
      externalRefs: [
        "https://segment.com/blog/allocation-efficiency-in-high-performance-go-services/",
      ],
      limitations: [
        "The Go compiler's escape analysis, not the value/pointer choice alone, determines heap allocation — a small struct passed by value can still escape to the heap if it's captured by a closure or returned via an interface, and a pointer receiver on a value that never escapes can stay stack-allocated, so this rule is a heuristic about typical cases, not a guarantee derivable from the receiver syntax alone.",
      ],
    },
    {
      id: "know-go-global-state",
      title: "Dependency Injection over Globals",
      body: "Global state makes concurrent tests impossible and hides dependencies. Struct-based dependency injection allows mocks to be easily swapped and isolates behavior safely.",
      externalRefs: ["https://go.dev/doc/effective_go#init"],
      limitations: [
        "A package-level variable that is write-once (assigned only in `init()` or `main()` before any goroutine starts) and never mutated afterward — such as a compiled `regexp.Regexp` or a loaded config struct treated as immutable — does not have the concurrent-mutation or hidden-dependency problems this rule targets, even though it is syntactically a global.",
      ],
    },
    {
      id: "know-go-context",
      title: "Context Propagation",
      body: "The `context` package carries deadlines, cancellation signals, and request-scoped values across API boundaries. Failing to propagate context means downstream requests cannot be cancelled when a client disconnects.",
      externalRefs: ["https://pkg.go.dev/context"],
      limitations: [
        "Pure, synchronous, CPU-only helper functions with no I/O, no blocking calls, and no sub-goroutines have no cancellation point to honor, so omitting a `context.Context` parameter is not a violation — the rule applies to functions that actually cross a blocking boundary (network, disk, channel receive), not to every function transitively called from a request handler.",
      ],
    },
  ],
};
