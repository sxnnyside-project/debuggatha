# Domain Glossary

Canonical vocabulary used across Debuggatha code, documentation, interfaces, and findings.

| Term | Replaces | Meaning |
|---|---|---|
| **Skill** | Prompt | Something Debuggatha knows how to inspect or evaluate, model-independent and deterministic. |
| **Review Pack** | Templates / Knowledge Packs | A cohesive bundle of skills, policies, rules, and domain knowledge consumed to review code. |
| **Review Policy** | Prompt (per run) | The auditable criteria behind a specific finding, assembled deterministically from packs and repository signals. |
| **Criteria Resolution** | Criteria Loading | Resolving a repository's concrete criteria model from manifests, configs, and docs, rather than just loading raw markdown. |
| **Identity** | Personality | One fixed, professional voice — no personality selector. |
| **Findings Ledger** | Database / Issue list | Persistent state machine tracking findings across their full lifecycle (open, acknowledged, resolved, dismissed, reopened). |
| **Repository Memory** | Settings | Persistent structured record of engineering decisions, accepted deviations, suppressions, and active conventions. |

