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
    },
    {
      id: "know-javascript-strict-equality",
      title: "Strict Equality vs Type Coercion",
      body: "Loose equality (`==`) forces JavaScript to perform type coercion, leading to notorious bugs like `[] == ![]` evaluating to `true`. Strict equality (`===`) checks both value and type, guaranteeing predictable logic.",
      externalRefs: [
        "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Equality_comparisons_and_sameness",
      ],
    },
    {
      id: "know-javascript-eval",
      title: "The Dangers of `eval`",
      body: "Calling `eval()` on arbitrary input allows attackers to execute arbitrary JavaScript within the application's context. This leads to XSS in browsers and RCE (Remote Code Execution) in Node.js.",
      externalRefs: [
        "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/eval#never_use_eval!",
      ],
    },
    {
      id: "know-javascript-template-literals",
      title: "Template Literals for Readability",
      body: "String concatenation using the `+` operator becomes unreadable across multiple lines and variables. Template literals provide clean interpolation, support multiline strings natively, and enable tagged templates.",
      externalRefs: [
        "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Template_literals",
      ],
    },
    {
      id: "know-javascript-native-prototypes",
      title: "Prototype Pollution and Collisions",
      body: "Modifying `Object.prototype` or `Array.prototype` alters the behavior of all objects/arrays across the entire runtime environment. This breaks third-party libraries and opens prototype pollution vectors.",
      externalRefs: ["https://eslint.org/docs/latest/rules/no-extend-native"],
    },
    {
      id: "know-javascript-arrow-functions",
      title: "Lexical `this` Binding",
      body: "Traditional `function() {}` callbacks bind their own `this` context, often requiring `.bind(this)` or `const self = this`. Arrow functions inherit `this` from the enclosing lexical scope, eliminating binding bugs.",
      externalRefs: [
        "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Functions/Arrow_functions",
      ],
    },
    {
      id: "know-javascript-spread",
      title: "Object Spread vs Object.assign",
      body: "Object spread syntax is declarative, concise, and natively supported. `Object.assign()` mutates the first argument unless an empty object is explicitly passed as the target, making it error-prone.",
      externalRefs: [
        "https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/Spread_syntax",
      ],
    },
    {
      id: "know-javascript-loop-closures",
      title: "Loop Closure Traps",
      body: "Creating functions inside loops that reference loop variables can lead to the classic 'closure in loop' bug, where every iteration captures the final mutated state of the variable instead of its per-iteration state.",
      externalRefs: ["https://eslint.org/docs/latest/rules/no-loop-func"],
    },
    {
      id: "know-javascript-require-await",
      title: "Unnecessary Microtasks",
      body: "Declaring a function as `async` without using `await` forces the runtime to wrap the return value in a Promise and queue it on the microtask queue, adding unnecessary execution overhead for synchronous logic.",
      externalRefs: ["https://eslint.org/docs/latest/rules/require-await"],
    },
  ],
};
