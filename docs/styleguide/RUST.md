# Rust style guide

<!-- How Rust is written and documented in this repository; for anyone changing the crates. -->

The API documentation is part of the code: `just docs` builds it with every warning denied, `just lint` runs clippy with `-D warnings`, and each library crate has `#![warn(missing_docs)]`. This guide says what those gates cannot: how a good doc comment reads.

---

## Layout

- A module is a folder named for what it holds, reached through its module file (`foo.rs` with `foo/`; never `mod.rs`).
- Unit tests are in the file under `#[cfg(test)]`. Integration tests are in `crates/*/tests/<concern>/main.rs`, one binary per concern.
- Visibility is the narrowest that works: `pub(super)` and `pub(in crate::detect)` before `pub`. `unreachable_pub` is a warning.
- Every clippy `allow` carries its reason on the same line or the line above. No `unwrap` or `expect` on a value the program does not control; one on an embedded constant carries an inline `debuggatha-ignore-next-line` with the reason.

## Documentation

- **Every module has a `//!` line** that says what it holds, as a sentence. A crate's `lib.rs` also lists its modules in a table.
- **Every public item has a `///` comment**: types, fields, variants, functions, constants. A field that restates its name (`/// The id.`) says what kind of id or where it comes from, or does not belong in the public API.
- **The first sentence is a summary** that fits one line and ends with a period. It says what the item is or does, not how: "The policy a review applies", not "Struct holding the policy".
- **Use the third person, present tense** for functions: "Reviews the files", not "Review the files" or "This function reviews".
- **Name the contract, not the implementation.** What is returned, what is refused, what is recorded. The reason a design is as it is belongs in an ADR, linked as `ADR-0005`.
- **`# Errors`** lists when a `Result` is an `Err`. **`# Panics`** lists when a function panics, and a function that never panics does not say so. **`# Safety`** does not occur: `unsafe_code` is forbidden.
- **Code in backticks**: every identifier, path, file name and command (`doc_markdown` enforces it). Product names that clippy mistakes for code are written in backticks too (`ESLint`, `PHPStan`).
- **Link with intra-doc links** (`[`Finding`]`, `[`pipeline::execute_review`]`); a broken link fails `just docs`.
- **Examples** are a doctest when a function is used on its own. A doc that would need a repository on disk is described in words.
- No comment narrates history, a ticket or what a change used to do. A comment that is still true after the next refactor belongs; one that is not does not.

## Errors and output

- A library returns a typed error that says what went wrong without a path or a stack trace a client should not see (`client_error`).
- `stdout` carries protocol or report data only; diagnostics go to `stderr`.

---

*Last updated: 2026-11-19*
