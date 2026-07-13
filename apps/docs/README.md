# @debuggatha/docs

Intentional placeholder — no documentation site exists here yet. This
workspace member exists so the monorepo's tooling (`turbo`, the root
`package.json` `workspaces` glob) has a stable place to add one later,
without a broader restructuring when that happens.

Confirmed still a placeholder as of the Epic 16A/16B Deferred Inventory
audit (2026-07). Every script in `package.json` is a no-op:

```json
"build": "echo 'docs site not yet implemented — see CLAUDE.md'"
```

If/when a real docs site is built here, replace this README along with
the actual implementation — don't treat this file as a spec, it only
records current (non-)status.
