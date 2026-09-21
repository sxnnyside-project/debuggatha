# Welcome to Debuggatha

Debuggatha is a senior engineering reviewer. It is not an autocomplete engine and not a chat wrapper.

It reads your repository first and enforces what it finds through **Review Packs**. You do not prompt it; you ask for a review.

## How it works

1. **Context discovery**: it reads your package manifests, config files, documentation, and directory structure to understand your stack.
2. **Policy assembly**: it matches your stack against its Review Packs to decide which rules apply.
3. **Review**: it checks your pending changes, a file, a selection, or the workspace, and runs the analyzers installed on your machine.
4. **Ledger**: findings are deduplicated and tracked over time, so you are not alerted twice for the same thing.
