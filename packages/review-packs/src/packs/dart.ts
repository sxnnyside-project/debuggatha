import type { ReviewPack } from "@debuggatha/knowledge-system";

export const dartPack: ReviewPack = {
  id: "debuggatha/dart",
  version: "1.0.0",
  kind: "stack",
  displayName: "Dart",
  dependsOn: [],
  rules: [
    {
      id: "prefer-const",
      packId: "debuggatha/dart",
      statement:
        "Use `const` for variables, constructors, and collections when their value is known at compile-time.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "dart" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-dart-const"],
      contradicts: undefined,
    },
    {
      id: "avoid-dynamic",
      packId: "debuggatha/dart",
      statement:
        "Avoid using the `dynamic` type. Use `Object?` if a variable can hold any value safely, or use generics.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "dart" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-dart-dynamic"],
      contradicts: undefined,
    },
    {
      id: "unawaited-futures",
      packId: "debuggatha/dart",
      statement:
        "Do not leave Futures unawaited. Use `await` or wrap intentionally fire-and-forget futures with `unawaited()`.",
      category: "security",
      appliesTo: { kind: "requires-language", language: "dart" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-dart-unawaited"],
      contradicts: undefined,
    },
    {
      id: "prefer-is-empty",
      packId: "debuggatha/dart",
      statement:
        "Use `.isEmpty` and `.isNotEmpty` on iterables instead of checking `.length == 0`.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "dart" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-dart-is-empty"],
      contradicts: undefined,
    },
    {
      id: "avoid-print",
      packId: "debuggatha/dart",
      statement:
        "Do not use `print()` in production code. Use the `logging` package or `debugPrint()` for debug builds.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "dart" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-dart-print"],
      contradicts: undefined,
    },
    {
      id: "prefer-final",
      packId: "debuggatha/dart",
      statement: "Mark variables as `final` if they are not reassigned after initialization.",
      category: "style",
      appliesTo: { kind: "requires-language", language: "dart" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-dart-final"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-dart-const",
      title: "Compile-Time Constants",
      body: "Dart canonicalizes `const` instances. Using `const` whenever possible reduces memory overhead and decreases Garbage Collection pressure, especially heavily in Flutter widget trees.",
      externalRefs: [
        "https://dart.dev/effective-dart/usage#prefer-using-const-for-instantiating-constant-constructors",
      ],
      limitations: [
        "A widget or value that depends on a runtime value (e.g. `DateTime.now()`, a value from `Theme.of(context)`, or a constructor argument that isn't itself const) cannot be marked `const` — flagging every non-const instantiation without checking whether its arguments are actually compile-time constants produces false positives.",
      ],
    },
    {
      id: "know-dart-dynamic",
      title: "Loss of Type Safety",
      body: "The `dynamic` type disables all static checking for that variable. Method calls will compile but may crash with `NoSuchMethodError` at runtime. `Object?` maintains static safety by forcing you to cast before invoking methods.",
      externalRefs: [
        "https://dart.dev/effective-dart/design#avoid-using-dynamic-unless-you-want-to-disable-static-checking",
      ],
      limitations: [
        "`dynamic` is the correct, idiomatic type at true JSON-decoding boundaries (`jsonDecode` returns `dynamic`) before the value is cast into a typed model — flagging `dynamic` at that specific boundary, rather than at the point it propagates further into typed code, produces noise rather than catching a real type-safety gap.",
      ],
    },
    {
      id: "know-dart-unawaited",
      title: "Swallowed Future Exceptions",
      body: "If an asynchronous function returns a Future that throws an exception, and that Future is not awaited, the exception is swallowed entirely. Using `unawaited(future)` explicitly documents that the lack of `await` is intentional.",
      externalRefs: ["https://dart-lang.github.io/linter/lints/unawaited_futures.html"],
      limitations: [
        "A Future stored and awaited later in the same scope (e.g. kicked off early for parallelism, then `await`ed just before its result is needed) is not actually unawaited — a rule that flags any Future-returning call not immediately preceded by `await` on that same line, without tracking whether the resulting Future is awaited elsewhere, will false-positive on this valid parallelization pattern.",
      ],
    },
    {
      id: "know-dart-is-empty",
      title: "Iterable Evaluation Speed",
      body: "Computing the length of an `Iterable` can be an O(N) operation if the iterable is lazy (e.g., a `.map()` or `.where()`). Checking `.isEmpty` is always an O(1) operation.",
      externalRefs: [
        "https://dart.dev/effective-dart/usage#prefer-using-isempty-or-isnotempty-for-iterables",
      ],
      limitations: [
        "On a `List` or other collection with an O(1) `.length` getter, `.length == 0` costs nothing extra — this is purely a style/consistency preference in that case, not a genuine performance fix, so flagging it as a performance issue overstates the impact for concrete list types versus lazy iterables.",
      ],
    },
    {
      id: "know-dart-print",
      title: "Print in Production",
      body: "The `print()` statement outputs to standard output in production, which can leak sensitive information or simply clutter device logs. Structured logging should always be used.",
      externalRefs: ["https://dart-lang.github.io/linter/lints/avoid_print.html"],
      limitations: [
        "`print()` inside test files, one-off CLI scripts, or build tooling (not the shipped app) is normal and not the production-logging concern this rule targets — flagging it there without distinguishing app source from test/tooling code produces false positives.",
      ],
    },
    {
      id: "know-dart-final",
      title: "Immutability Intent",
      body: "Using `final` clearly communicates to readers and the compiler that a variable's binding will not change. This decreases cognitive load when reading functions.",
      externalRefs: ["https://dart.dev/effective-dart/usage#prefer-making-declarations-final"],
      limitations: [
        "A loop-control variable or a field mutated by a Flutter `State` class's `setState`-driven lifecycle needs to remain mutable (`var`) by design — flagging any non-final local or field without recognizing intentional mutability patterns like these produces false positives.",
      ],
    },
  ],
};
