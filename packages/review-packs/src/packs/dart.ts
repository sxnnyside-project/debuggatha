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
    },
    {
      id: "know-dart-dynamic",
      title: "Loss of Type Safety",
      body: "The `dynamic` type disables all static checking for that variable. Method calls will compile but may crash with `NoSuchMethodError` at runtime. `Object?` maintains static safety by forcing you to cast before invoking methods.",
      externalRefs: [
        "https://dart.dev/effective-dart/design#avoid-using-dynamic-unless-you-want-to-disable-static-checking",
      ],
    },
    {
      id: "know-dart-unawaited",
      title: "Swallowed Future Exceptions",
      body: "If an asynchronous function returns a Future that throws an exception, and that Future is not awaited, the exception is swallowed entirely. Using `unawaited(future)` explicitly documents that the lack of `await` is intentional.",
      externalRefs: ["https://dart-lang.github.io/linter/lints/unawaited_futures.html"],
    },
    {
      id: "know-dart-is-empty",
      title: "Iterable Evaluation Speed",
      body: "Computing the length of an `Iterable` can be an O(N) operation if the iterable is lazy (e.g., a `.map()` or `.where()`). Checking `.isEmpty` is always an O(1) operation.",
      externalRefs: [
        "https://dart.dev/effective-dart/usage#prefer-using-isempty-or-isnotempty-for-iterables",
      ],
    },
    {
      id: "know-dart-print",
      title: "Print in Production",
      body: "The `print()` statement outputs to standard output in production, which can leak sensitive information or simply clutter device logs. Structured logging should always be used.",
      externalRefs: ["https://dart-lang.github.io/linter/lints/avoid_print.html"],
    },
    {
      id: "know-dart-final",
      title: "Immutability Intent",
      body: "Using `final` clearly communicates to readers and the compiler that a variable's binding will not change. This decreases cognitive load when reading functions.",
      externalRefs: ["https://dart.dev/effective-dart/usage#prefer-making-declarations-final"],
    },
  ],
};
