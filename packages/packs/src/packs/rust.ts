import type { ReviewPack } from "@debuggatha/core";

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
      limitations: [
        "`.unwrap()`/`.expect()` in `#[test]` functions, `main()` prototypes, or on an invariant the type system already guarantees can never fail (e.g. unwrapping a regex compiled from a `const` literal known valid at compile time) is idiomatic, not a defect — this rule targets domain/production logic where the `None`/`Err` case is a real runtime possibility.",
      ],
    },
    {
      id: "know-rust-clone",
      title: "The Cost of `.clone()`",
      body: "`.clone()` performs a deep heap allocation for types like `String` or `Vec`. Overusing it to satisfy the borrow checker masks underlying architectural flaws and causes significant performance degradation.",
      externalRefs: ["https://rust-lang.github.io/api-guidelines/flexibility.html"],
      limitations: [
        "`.clone()` on a `Copy` type (integers, small structs deriving `Copy`) or on an `Rc`/`Arc` (a cheap refcount bump, not a deep copy) is effectively free — flagging every `.clone()` call by text match without checking the underlying type's cost profile produces false positives on these cases.",
      ],
    },
    {
      id: "know-rust-string-slices",
      title: "Flexibility of String Slices",
      body: "Accepting `&str` instead of `String` allows the caller to pass either a string literal (`&'static str`), a borrowed `String`, or any type implementing `AsRef<str>` without forcing an allocation.",
      externalRefs: [
        "https://doc.rust-lang.org/book/ch04-03-slices.html#string-slices-as-parameters",
      ],
      limitations: [
        "A function that stores the string in a struct field, spawns it into another thread, or otherwise needs `'static`/owned data genuinely requires `String` (or an explicit `Cow<str>`) — flagging every owned-`String` parameter as suboptimal ignores functions where ownership is a real requirement, not just unaddressed borrow-checker friction.",
      ],
    },
    {
      id: "know-rust-panic",
      title: "Panics in Library Code",
      body: "Libraries should never panic, as it removes the application author's ability to gracefully recover. Always return a `Result` so the consumer decides whether the error is fatal.",
      externalRefs: ["https://rust-lang.github.io/api-guidelines/dependability.html#c-panic"],
      limitations: [
        "A `panic!` guarding a genuine programmer-error invariant (an internal debug_assert-style sanity check, or a documented precondition violation on a `pub` function per Rust API guidelines' C-PANIC) is an accepted exception, not a defect — this rule targets panics that surface expected, recoverable runtime failure conditions to library consumers.",
      ],
    },
    {
      id: "know-rust-clippy",
      title: "Clippy as the Standard",
      body: "Clippy catches common mistakes and unidiomatic code that the compiler ignores. Suppressing Clippy warnings hides tech debt and prevents the codebase from adopting established community idioms.",
      externalRefs: ["https://github.com/rust-lang/rust-clippy"],
      limitations: [
        'Some `clippy::pedantic`/`clippy::nursery` lints are opinionated or have a known high false-positive rate on generic/macro-heavy code — a blanket "any `#[allow(clippy::...)]` is a violation" check can\'t distinguish those cases from suppressing a legitimate correctness lint like `clippy::correctness`.',
      ],
    },
    {
      id: "know-rust-iterators",
      title: "Zero-Cost Abstraction Iterators",
      body: "Rust's iterators are zero-cost abstractions that compile down to loop equivalents without the bounds-checking overhead of explicit indexing. They are both more ergonomic and often faster.",
      externalRefs: ["https://doc.rust-lang.org/book/ch13-02-iterators.html"],
      limitations: [
        "An explicit `for` loop is still the right call when the body needs early-exit control flow with a value (`break value`), mutates multiple unrelated variables per iteration, or the iterator-chain equivalent would need `.enumerate().zip(...)` gymnastics that hurt readability more than they gain — this rule is a default preference, not an absolute one.",
      ],
    },
    {
      id: "know-rust-traits",
      title: "Standard Trait Implementations",
      body: "Types without `Debug` cannot be easily printed. Types without `Clone` force callers into reference lifetimes. Deriving standard traits ensures your types are ergonomic for downstream consumers.",
      externalRefs: ["https://rust-lang.github.io/api-guidelines/interoperability.html"],
      limitations: [
        "Types wrapping a non-`Clone` resource (a raw socket, a mutex guard, a unique handle) or containing sensitive data where `Debug` output could leak secrets into logs are correctly excluded from these derives — flagging every public struct missing `Debug`/`Clone` ignores types where deriving them would be actively wrong.",
      ],
    },
    {
      id: "know-rust-unsafe",
      title: "The Danger of Unsafe Rust",
      body: "The `unsafe` keyword turns off Rust's memory safety guarantees (borrow checking, null pointer dereferences). Improper use leads to undefined behavior, buffer overflows, and segmentation faults.",
      externalRefs: ["https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html"],
      limitations: [
        "A small, well-documented `unsafe` block wrapped by a safe public API (the standard pattern used by `Vec`, `str`, and most FFI bindings) is the idiomatic way to build sound abstractions — flagging the mere presence of `unsafe` without checking whether its invariants are documented and upheld conflates necessary, correctly-encapsulated unsafe code with genuinely risky use.",
      ],
    },
  ],
};
