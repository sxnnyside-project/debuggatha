# TypeScript style guide

<!-- How the VS Code extension is written and documented; for anyone changing apps/vscode. -->

The extension is the only TypeScript left. It is Node and pnpm, strict TypeScript, formatted and linted by Biome (`just vscode-check`). Documentation follows [TSDoc](https://tsdoc.org).

---

## Layout

- Modules are named for what they hold, in kebab-case or camelCase as the folder already does, and a module imports only what it needs. Tests sit beside the code as `<name>.test.ts`; those that need a real VS Code are `<name>.itest.ts`.
- `src/contracts` is generated from `docs/schemas`; it is never edited by hand.
- The extension holds no review logic. It asks the `debuggatha` binary over Protocol v1 and shows the answer with native VS Code parts.

## Documentation

- **Every file starts with a `/** ... @packageDocumentation */` comment** that says what the module is for.
- **Every exported declaration has a `/** */` comment**: functions, classes, interfaces and their public members, type aliases and constants. Private helpers are documented when their reason is not obvious.
- **The first sentence is a summary** that ends with a period and says what the item is or does ("The findings of the ledger, indexed by file"), not how.
- **Use `@param name - description`** (with the hyphen) only for a parameter whose meaning is not clear from its name and type; never repeat the type, TypeScript already has it. Use `@returns` when the result is not obvious, and `@throws` for an error a caller must handle.
- **Link with `{@link Name}`** for other declarations, and put code in backticks.
- A comment that restates the name (`/** The id. */`) is removed or made to say something.
- No comment narrates history or what a change used to do.

## Trust

- The extension runs only built-in detectors in an untrusted workspace, and the settings that start a program are machine scope. A change that touches either says so in its comment.

---

*Last updated: 2026-11-19*
