import type { ReviewPack } from "@debuggatha/knowledge-system";

export const javascriptPack: ReviewPack = {
  id: "debuggatha/javascript",
  version: "1.0.0",
  kind: "stack",
  displayName: "JavaScript",
  dependsOn: [],
  rules: [
    {
      id: "no-var",
      packId: "debuggatha/javascript",
      statement:
        "Use `let` or `const` instead of `var` to ensure block scoping and prevent hoisting bugs.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "javascript" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-javascript-block-scoping"],
      contradicts: undefined,
    },
    {
      id: "eqeqeq",
      packId: "debuggatha/javascript",
      statement:
        "Always use strict equality (`===` and `!==`) instead of loose equality (`==` and `!=`).",
      category: "security",
      appliesTo: { kind: "requires-language", language: "javascript" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-javascript-strict-equality"],
      contradicts: undefined,
    },
    {
      id: "no-eval",
      packId: "debuggatha/javascript",
      statement:
        "Never use `eval()` or pass strings to `setTimeout`/`setInterval`. This is a severe security vulnerability.",
      category: "security",
      appliesTo: { kind: "requires-language", language: "javascript" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-javascript-eval"],
      contradicts: undefined,
    },
    {
      id: "prefer-template-literals",
      packId: "debuggatha/javascript",
      statement:
        "Use template literals (`\\```) for string interpolation rather than string concatenation (`+`).",
      category: "style",
      appliesTo: { kind: "requires-language", language: "javascript" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-javascript-template-literals"],
      contradicts: undefined,
    },
    {
      id: "no-extend-native",
      packId: "debuggatha/javascript",
      statement:
        "Do not add properties or methods to the prototypes of native objects like `Object`, `Array`, or `String`.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "javascript" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-javascript-native-prototypes"],
      contradicts: undefined,
    },
    {
      id: "prefer-arrow-callbacks",
      packId: "debuggatha/javascript",
      statement: "Use arrow functions for callbacks to preserve the lexical scope of `this`.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "javascript" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-javascript-arrow-functions"],
      contradicts: undefined,
    },
    {
      id: "no-implied-eval",
      packId: "debuggatha/javascript",
      statement:
        "Avoid using `new Function()` with dynamic strings, as it operates identically to `eval()` and opens XSS vectors.",
      category: "security",
      appliesTo: { kind: "requires-language", language: "javascript" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-javascript-eval"],
      contradicts: undefined,
    },
    {
      id: "prefer-object-spread",
      packId: "debuggatha/javascript",
      statement:
        "Use object spread syntax (`{ ...obj }`) instead of `Object.assign()` for shallow copying and merging.",
      category: "style",
      appliesTo: { kind: "requires-language", language: "javascript" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-javascript-spread"],
      contradicts: undefined,
    },
    {
      id: "no-loop-func",
      packId: "debuggatha/javascript",
      statement:
        "Avoid defining functions inside loops unless they exclusively reference block-scoped variables, otherwise loop closures will capture the final state.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "javascript" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-javascript-loop-closures"],
      contradicts: undefined,
    },
    {
      id: "require-await",
      packId: "debuggatha/javascript",
      statement:
        "Functions declared `async` must contain an `await` expression unless explicitly implementing an interface that requires a Promise.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "javascript" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-javascript-require-await"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-javascript-block-scoping",
      title: "Block Scoping with `let` and `const`",
      body: "Variables declared with `var` are function-scoped and hoisted to the top of their enclosing function. This leads to unpredictable variable capturing, especially in loops and callbacks. `let` and `const` enforce block scope.",
      externalRefs: [
        "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/let",
      ],
      limitations: [
        "Legacy code intentionally relying on `var`'s function-scoped hoisting (e.g. a variable deliberately declared once and reused across nested `if` blocks) is not automatically buggy — flagging every `var` occurrence without checking whether hoisting is actually being exploited unsafely produces noise on code that works correctly as written.",
      ],
    },
    {
      id: "know-javascript-strict-equality",
      title: "Strict Equality vs Type Coercion",
      body: "Loose equality (`==`) forces JavaScript to perform type coercion, leading to notorious bugs like `[] == ![]` evaluating to `true`. Strict equality (`===`) checks both value and type, guaranteeing predictable logic.",
      externalRefs: [
        "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Equality_comparisons_and_sameness",
      ],
      limitations: [
        "`value == null` is a widely-used, intentional idiom to check for both `null` and `undefined` in one comparison — flagging it as a violation of strict-equality misses that this specific loose-equality case is a deliberate, well-understood pattern, not an accidental coercion bug.",
      ],
    },
    {
      id: "know-javascript-eval",
      title: "The Dangers of `eval`",
      body: "Calling `eval()` on arbitrary input allows attackers to execute arbitrary JavaScript within the application's context. This leads to XSS in browsers and RCE (Remote Code Execution) in Node.js.",
      externalRefs: [
        "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/eval#never_use_eval!",
      ],
      limitations: [
        "`eval()` called only on a hardcoded, developer-authored string literal with no user input reachable in its construction (rare, but seen in some build-tool or REPL internals) is not the RCE/XSS vector this entry describes — the risk is specifically evaluating attacker-influenced input, not the mere presence of the `eval` token.",
      ],
    },
    {
      id: "know-javascript-template-literals",
      title: "Template Literals for Readability",
      body: "String concatenation using the `+` operator becomes unreadable across multiple lines and variables. Template literals provide clean interpolation, support multiline strings natively, and enable tagged templates.",
      externalRefs: [
        "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Template_literals",
      ],
      limitations: [
        "Simple two-operand concatenation used for performance-sensitive hot paths (string building in a tight loop) can be marginally faster than template literal interpolation in some engines — flagging every `+` concatenation as a pure style violation ignores cases where concatenation was a deliberate micro-optimization, not an oversight.",
      ],
    },
    {
      id: "know-javascript-native-prototypes",
      title: "Prototype Pollution and Collisions",
      body: "Modifying `Object.prototype` or `Array.prototype` alters the behavior of all objects/arrays across the entire runtime environment. This breaks third-party libraries and opens prototype pollution vectors.",
      externalRefs: ["https://eslint.org/docs/latest/rules/no-extend-native"],
      limitations: [
        "Deliberately polyfilling a missing standard method (e.g. adding `Array.prototype.at` for an environment that predates it) via a well-scoped, spec-compliant polyfill is a legitimate, common pattern — flagging every native-prototype extension identically to an ad-hoc custom method addition ignores the difference between polyfilling and true prototype pollution.",
      ],
    },
    {
      id: "know-javascript-arrow-functions",
      title: "Lexical `this` Binding",
      body: "Traditional `function() {}` callbacks bind their own `this` context, often requiring `.bind(this)` or `const self = this`. Arrow functions inherit `this` from the enclosing lexical scope, eliminating binding bugs.",
      externalRefs: [
        "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Functions/Arrow_functions",
      ],
      limitations: [
        "Object methods and class methods intentionally rely on dynamic `this` binding at call time (e.g. an event handler expected to receive the DOM element as `this`, or a method meant to be called via `.call()`/`.apply()` with a different receiver) — converting these to arrow functions would silently break that binding, so flagging every non-arrow callback uniformly is unsafe.",
      ],
    },
    {
      id: "know-javascript-spread",
      title: "Object Spread vs Object.assign",
      body: "Object spread syntax is declarative, concise, and natively supported. `Object.assign()` mutates the first argument unless an empty object is explicitly passed as the target, making it error-prone.",
      externalRefs: [
        "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Spread_syntax",
      ],
      limitations: [
        "`Object.assign()` is still the correct choice when merging into an existing object in place is actually intended (e.g. mutating a shared config object that other code holds a reference to) — flagging it uniformly as inferior to spread ignores that spread always creates a new object, which is a different, not strictly better, semantic.",
      ],
    },
    {
      id: "know-javascript-loop-closures",
      title: "Loop Closure Traps",
      body: "Creating functions inside loops that reference loop variables can lead to the classic 'closure in loop' bug, where every iteration captures the final mutated state of the variable instead of its per-iteration state.",
      externalRefs: ["https://eslint.org/docs/latest/rules/no-loop-func"],
      limitations: [
        "A function defined inside a `for...of` or `for...in` loop that only closes over `let`-scoped, per-iteration bindings does not have the classic closure-capture bug at all, since `let` creates a fresh binding each iteration — flagging any function literal inside any loop, without checking whether the loop variable is `var` versus block-scoped, produces false positives on already-safe code.",
      ],
    },
    {
      id: "know-javascript-require-await",
      title: "Unnecessary Microtasks",
      body: "Declaring a function as `async` without using `await` forces the runtime to wrap the return value in a Promise and queue it on the microtask queue, adding unnecessary execution overhead for synchronous logic.",
      externalRefs: ["https://eslint.org/docs/latest/rules/require-await"],
      limitations: [
        "An `async` function that must satisfy an interface or abstract base class expecting a Promise-returning method (e.g. implementing a shared `Repository` interface where some implementations are genuinely synchronous) legitimately has no `await` — the rule's own statement carves this out, but a scan checking syntax alone can't verify interface conformance and may still flag it.",
      ],
    },
  ],
};
