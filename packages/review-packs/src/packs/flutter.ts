import type { ReviewPack } from "@debuggatha/knowledge-system";

export const flutterPack: ReviewPack = {
  id: "debuggatha/flutter",
  version: "1.0.0",
  kind: "stack",
  displayName: "Flutter",
  dependsOn: [],
  rules: [
    {
      id: "prefer-stateless",
      packId: "debuggatha/flutter",
      statement:
        "Prefer `StatelessWidget` over `StatefulWidget`. Only use `StatefulWidget` when the widget itself must mutate local state.",
      category: "architecture",
      appliesTo: { kind: "requires-framework", framework: "flutter" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-flutter-stateless"],
      contradicts: undefined,
    },
    {
      id: "use-const-constructors",
      packId: "debuggatha/flutter",
      statement:
        "Prefix widget constructors with `const` wherever possible to prevent the widget from being rebuilt during parent renders.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "flutter" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-flutter-const"],
      contradicts: undefined,
    },
    {
      id: "avoid-heavy-build",
      packId: "debuggatha/flutter",
      statement:
        "Avoid placing computationally expensive operations or synchronous network/disk I/O inside the `build()` method.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "flutter" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-flutter-build"],
      contradicts: undefined,
    },
    {
      id: "dispose-controllers",
      packId: "debuggatha/flutter",
      statement:
        "Always dispose `TextEditingController`, `AnimationController`, and `ScrollController` in the `dispose()` method of the `State` object.",
      category: "performance",
      appliesTo: { kind: "requires-framework", framework: "flutter" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-flutter-dispose"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-flutter-stateless",
      title: "Stateless Widget Efficiency",
      body: "`StatelessWidget` requires less memory and lifecycle overhead than `StatefulWidget`. If state is managed by a state management library (like Riverpod or Bloc), the widget itself rarely needs its own mutable state.",
      externalRefs: ["https://docs.flutter.dev/perf/best-practices#control-build-cost"],
    },
    {
      id: "know-flutter-const",
      title: "Const Widgets and the Element Tree",
      body: "Flutter compares the old and new widget trees to determine what to paint. When a widget is marked as `const`, Flutter knows it cannot change, allowing it to short-circuit the comparison and reuse the existing element and render object.",
      externalRefs: ["https://dart-lang.github.io/linter/lints/prefer_const_constructors.html"],
    },
    {
      id: "know-flutter-build",
      title: "The Build Method is a Hot Path",
      body: "The `build()` method can be called up to 60 times a second during animations. Placing heavy synchronous logic inside it guarantees UI stuttering (jank).",
      externalRefs: ["https://docs.flutter.dev/perf/best-practices#avoid-doing-work-in-build"],
    },
    {
      id: "know-flutter-dispose",
      title: "Controller Memory Leaks",
      body: "Controllers hook into native layers or maintain complex observable state. Failing to call `.dispose()` when a widget is destroyed causes permanent memory leaks.",
      externalRefs: ["https://api.flutter.dev/flutter/widgets/TextEditingController-class.html"],
    },
  ],
};
