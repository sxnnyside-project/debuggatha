import type { ReviewPack } from "@debuggatha/knowledge-system";

export const kotlinPack: ReviewPack = {
  id: "debuggatha/kotlin",
  version: "1.0.0",
  kind: "stack",
  displayName: "Kotlin",
  dependsOn: [],
  rules: [
    {
      id: "no-double-bang",
      packId: "debuggatha/kotlin",
      statement:
        "Never use the `!!` (not-null assertion) operator. Use safe calls `?.`, Elvis operator `?:`, or smart casts instead.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "kotlin" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-kotlin-null-safety"],
      contradicts: undefined,
    },
    {
      id: "prefer-val-over-var",
      packId: "debuggatha/kotlin",
      statement: "Always use `val` for read-only variables unless mutation is strictly required.",
      category: "style",
      appliesTo: { kind: "requires-language", language: "kotlin" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-kotlin-immutability"],
      contradicts: undefined,
    },
    {
      id: "use-sealed-classes",
      packId: "debuggatha/kotlin",
      statement:
        "Use `sealed class` or `sealed interface` for representing restricted class hierarchies instead of open classes or basic enums when state is involved.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "kotlin" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-kotlin-sealed"],
      contradicts: undefined,
    },
    {
      id: "exhaustive-when",
      packId: "debuggatha/kotlin",
      statement:
        "Ensure `when` expressions on sealed classes or enums are exhaustive. Do not use generic `else` branches if all known cases can be mapped.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "kotlin" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-kotlin-exhaustive-when"],
      contradicts: undefined,
    },
    {
      id: "avoid-companion-object-state",
      packId: "debuggatha/kotlin",
      statement:
        "Do not store mutable state in `companion object` blocks. They are singletons and will cause race conditions in multithreaded environments.",
      category: "security",
      appliesTo: { kind: "requires-language", language: "kotlin" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-kotlin-companion"],
      contradicts: undefined,
    },
    {
      id: "inline-functions-for-lambdas",
      packId: "debuggatha/kotlin",
      statement:
        "Use the `inline` modifier for higher-order functions that take lambdas to prevent runtime allocation overhead of function objects.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "kotlin" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-kotlin-inline"],
      contradicts: undefined,
    },
    {
      id: "suspend-function-cancellation",
      packId: "debuggatha/kotlin",
      statement:
        "Ensure long-running loops in `suspend` functions periodically check for cancellation (e.g., `yield()` or `isActive`).",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "kotlin" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-kotlin-coroutines"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-kotlin-null-safety",
      title: "Subverting Null Safety",
      body: "The `!!` operator forcefully converts any value to a non-null type. If the value is null, it instantly throws a `NullPointerException`. This completely circumvents Kotlin's primary safety feature.",
      externalRefs: ["https://kotlinlang.org/docs/null-safety.html#the-operator"],
      limitations: [
        "In a unit test asserting that a value is non-null before further assertions run, `!!` is an accepted idiom (a null there should fail loudly, immediately) — flagging every `!!` occurrence identically whether in production request-handling code or in test setup treats a deliberate fail-fast assertion the same as a production null-safety violation.",
      ],
    },
    {
      id: "know-kotlin-immutability",
      title: "Immutable by Default",
      body: "Favoring `val` over `var` makes code easier to reason about, safer in concurrent contexts, and reduces the scope for state-related bugs.",
      externalRefs: ["https://kotlinlang.org/docs/coding-conventions.html#immutability"],
      limitations: [
        "A loop accumulator, builder-pattern intermediate variable, or a field genuinely reassigned across a state machine's transitions needs `var` by design — flagging every `var` declaration without checking whether reassignment is actually algorithmically necessary produces false positives on legitimately mutable state.",
      ],
    },
    {
      id: "know-kotlin-sealed",
      title: "Sealed Classes vs Enums",
      body: "Enums allow a restricted set of instances, but they cannot hold different state per instance. Sealed classes allow subclasses to have multiple instances, each with their own state, making them ideal for representing algebraic data types like `Result<T>`.",
      externalRefs: ["https://kotlinlang.org/docs/sealed-classes.html"],
      limitations: [
        "A truly fixed, stateless set of named constants (e.g. `enum class Direction { NORTH, SOUTH, EAST, WEST }`) has no per-instance state to represent, so an `enum` is the correct, idiomatic choice there — flagging every enum as a candidate for conversion to a sealed class ignores that sealed classes solve a problem (per-variant state) the enum doesn't have.",
      ],
    },
    {
      id: "know-kotlin-exhaustive-when",
      title: "Exhaustive Pattern Matching",
      body: "When you use an `else` branch on a sealed class hierarchy, the compiler stops warning you when new subclasses are added. Removing the `else` branch forces compile-time checking of all cases.",
      externalRefs: ["https://kotlinlang.org/docs/control-flow.html#when-expression"],
      limitations: [
        "A `when` over a sealed hierarchy defined in a third-party library the codebase doesn't control (where new subclasses could be added by an upstream release without warning) may legitimately need a defensive `else` branch to avoid a runtime `NoWhenBranchMatchedException` on an unrecognized future subtype — flagging `else` as always avoidable ignores hierarchies not owned by the reviewed codebase.",
      ],
    },
    {
      id: "know-kotlin-companion",
      title: "Singleton Mutation",
      body: "Companion objects are initialized statically. Storing `var` properties inside them creates global mutable state, leading to cross-contamination between tests and threading issues in production.",
      externalRefs: ["https://kotlinlang.org/docs/object-declarations.html#companion-objects"],
      limitations: [
        "A companion object `var` guarded by proper synchronization (e.g. `@Volatile` plus double-checked locking for a lazy singleton, or an `AtomicReference`) is a correctly-implemented thread-safe pattern, not the race-condition case this rule targets — flagging every mutable companion property regardless of synchronization produces false positives on deliberately hardened singleton code.",
      ],
    },
    {
      id: "know-kotlin-inline",
      title: "Inline Functions",
      body: "By default, lambdas in Kotlin compile to anonymous inner classes. Calling a lambda allocates an object. `inline` tells the compiler to paste the function's bytecode directly at the call site, eliminating the allocation.",
      externalRefs: ["https://kotlinlang.org/docs/inline-functions.html"],
      limitations: [
        "A large, rarely-called higher-order function marked `inline` bloats bytecode at every call site (code-size/JIT-cacheability tradeoff) without a meaningful runtime win, and `inline` is disallowed entirely on public API functions that need binary stability across module boundaries — flagging every non-inlined lambda-accepting function as a missed optimization ignores these cases where inlining is actively undesirable or impossible.",
      ],
    },
    {
      id: "know-kotlin-coroutines",
      title: "Cooperative Cancellation",
      body: "Kotlin Coroutines are cooperatively cancelled. If a `suspend` function contains a tight loop (e.g., reading a file or processing a large collection) and never calls a suspending function like `yield()` or checks `isActive`, it cannot be cancelled by the parent scope.",
      externalRefs: [
        "https://kotlinlang.org/docs/cancellation-and-timeouts.html#cancellation-is-cooperative",
      ],
      limitations: [
        "A loop that already calls a genuinely suspending function on every iteration (e.g. an actual network or database call per item) has an implicit cancellation checkpoint at that suspension point already — flagging the absence of an explicit `yield()`/`isActive` check without recognizing an existing suspension point inside the loop produces a false positive.",
      ],
    },
  ],
};
