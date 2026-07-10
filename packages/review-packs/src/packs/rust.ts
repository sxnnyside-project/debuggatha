import type { ReviewPack } from "@debuggatha/knowledge-system";

export const rustPack: ReviewPack = {
  id: "debuggatha/rust",
  version: "1.0.0",
  kind: "stack",
  displayName: "Rust",
  dependsOn: [],
  rules: [
    {
      id: "no-unwrap-expect",
      packId: "debuggatha/rust",
      statement:
        "Do not use `.unwrap()` or `.expect()` in domain logic or production code. Always handle `Result` or `Option` explicitly using pattern matching or the `?` operator.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "rust" },
      defaultSeverity: "high",
      knowledgeRefs: ["know-rust-error-handling"],
      contradicts: undefined,
    },
    {
      id: "clone-copy-semantics",
      packId: "debuggatha/rust",
      statement:
        "Avoid calling `.clone()` in hot paths. Prefer borrowing (`&T`) where ownership is not strictly required.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "rust" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-rust-clone"],
      contradicts: undefined,
    },
    {
      id: "prefer-str-over-string",
      packId: "debuggatha/rust",
      statement:
        "Use string slices (`&str`) as function parameters instead of owned `String` types unless the function explicitly requires ownership.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "rust" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-rust-string-slices"],
      contradicts: undefined,
    },
    {
      id: "no-panic",
      packId: "debuggatha/rust",
      statement: "Never use the `panic!` macro in library code. Propagate errors to the caller.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "rust" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-rust-panic"],
      contradicts: undefined,
    },
    {
      id: "use-clippy",
      packId: "debuggatha/rust",
      statement:
        "Address all warnings emitted by `cargo clippy`. Do not use `#[allow(clippy::...)]` without explicit justification.",
      category: "style",
      appliesTo: { kind: "requires-language", language: "rust" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-rust-clippy"],
      contradicts: undefined,
    },
    {
      id: "prefer-iterators",
      packId: "debuggatha/rust",
      statement:
        "Use iterators instead of explicit `for` loops when chaining transformations (e.g., `map`, `filter`, `fold`) for zero-cost abstractions.",
      category: "performance",
      appliesTo: { kind: "requires-language", language: "rust" },
      defaultSeverity: "low",
      knowledgeRefs: ["know-rust-iterators"],
      contradicts: undefined,
    },
    {
      id: "derive-traits",
      packId: "debuggatha/rust",
      statement:
        "Always implement or derive standard traits (`Debug`, `Clone`, `Eq`, `PartialEq`) for public structs and enums when applicable.",
      category: "architecture",
      appliesTo: { kind: "requires-language", language: "rust" },
      defaultSeverity: "medium",
      knowledgeRefs: ["know-rust-traits"],
      contradicts: undefined,
    },
    {
      id: "no-unsafe-blocks",
      packId: "debuggatha/rust",
      statement:
        "Avoid `unsafe` blocks unless interfacing with FFI or proving extreme performance bottlenecks. If used, document the safety invariants thoroughly.",
      category: "security",
      appliesTo: { kind: "requires-language", language: "rust" },
      defaultSeverity: "critical",
      knowledgeRefs: ["know-rust-unsafe"],
      contradicts: undefined,
    },
  ],
  knowledge: [
    {
      id: "know-rust-error-handling",
      title: "Graceful Error Handling via `Result`",
      body: "Calling `.unwrap()` or `.expect()` will panic and crash the thread if the value is `None` or `Err`. Rust's type system is designed to force explicit error handling. The `?` operator ergonomically propagates errors up the call stack.",
      externalRefs: ["https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html"],
    },
    {
      id: "know-rust-clone",
      title: "The Cost of `.clone()`",
      body: "`.clone()` performs a deep heap allocation for types like `String` or `Vec`. Overusing it to satisfy the borrow checker masks underlying architectural flaws and causes significant performance degradation.",
      externalRefs: ["https://rust-lang.github.io/api-guidelines/flexibility.html"],
    },
    {
      id: "know-rust-string-slices",
      title: "Flexibility of String Slices",
      body: "Accepting `&str` instead of `String` allows the caller to pass either a string literal (`&'static str`), a borrowed `String`, or any type implementing `AsRef<str>` without forcing an allocation.",
      externalRefs: [
        "https://doc.rust-lang.org/book/ch04-03-slices.html#string-slices-as-parameters",
      ],
    },
    {
      id: "know-rust-panic",
      title: "Panics in Library Code",
      body: "Libraries should never panic, as it removes the application author's ability to gracefully recover. Always return a `Result` so the consumer decides whether the error is fatal.",
      externalRefs: ["https://rust-lang.github.io/api-guidelines/dependability.html#c-panic"],
    },
    {
      id: "know-rust-clippy",
      title: "Clippy as the Standard",
      body: "Clippy catches common mistakes and unidiomatic code that the compiler ignores. Suppressing Clippy warnings hides tech debt and prevents the codebase from adopting established community idioms.",
      externalRefs: ["https://github.com/rust-lang/rust-clippy"],
    },
    {
      id: "know-rust-iterators",
      title: "Zero-Cost Abstraction Iterators",
      body: "Rust's iterators are zero-cost abstractions that compile down to loop equivalents without the bounds-checking overhead of explicit indexing. They are both more ergonomic and often faster.",
      externalRefs: ["https://doc.rust-lang.org/book/ch13-02-iterators.html"],
    },
    {
      id: "know-rust-traits",
      title: "Standard Trait Implementations",
      body: "Types without `Debug` cannot be easily printed. Types without `Clone` force callers into reference lifetimes. Deriving standard traits ensures your types are ergonomic for downstream consumers.",
      externalRefs: ["https://rust-lang.github.io/api-guidelines/interoperability.html"],
    },
    {
      id: "know-rust-unsafe",
      title: "The Danger of Unsafe Rust",
      body: "The `unsafe` keyword turns off Rust's memory safety guarantees (borrow checking, null pointer dereferences). Improper use leads to undefined behavior, buffer overflows, and segmentation faults.",
      externalRefs: ["https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html"],
    },
  ],
};
