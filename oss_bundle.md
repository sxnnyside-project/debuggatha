# 🟢 OSS Bundle — Sxnnyside Project

> Perfil: `public-oss` · Licencia: **MIT** · Set completo, todos los archivos aplican.
> Fuente: [Sxnnyside Ecosystem Standard Documentacion GitHub](https://app.notion.com/p/3649f9a551d2806e8a9ed6b6018c4bf1)

## Leyenda de variables
- `🔁 REALM` → cambia según la rama/realm a la que pertenece el proyecto (fijo mientras el realm no cambie).
- `🔁 PROYECTO` → cambia por repositorio específico, incluso dentro del mismo realm.
- Sin marca → valor fijo del ecosistema, no cambia nunca.

| Variable | Tipo | Descripción |
|---|---|---|
| `{{PROJECT_NAME}}` | 🔁 PROYECTO | Nombre del proyecto |
| `{{PROJECT_SLUG}}` | 🔁 PROYECTO | kebab-case, usado en el banner |
| `{{REALM_ATTRIBUTION}}` | 🔁 REALM | Ej. `A CoreRed Project` / `An Acid Savior Release` |
| `{{GITHUB_ORG}}` | 🔁 REALM | Organización de GitHub del Realm |
| `{{GITHUB_USER}}` | fijo | Usuario maintainer — siempre el mismo |
| `{{REPO_NAME}}` | 🔁 PROYECTO | Nombre del repositorio |
| `{{YEAR}}` | 🔁 PROYECTO | Año del release inicial |
| `{{VERSION}}` | 🔁 PROYECTO | `MAJOR.MINOR.PATCH` |
| `{{DISCORD_INVITE}}` | fijo (o 🔁 REALM si hay Discords separados por realm) | Invite code del servidor |
| `BRANCH` (en LICENSE) | 🔁 REALM | Nombre de la rama para el copyright — vacío si es rama principal |

---

## README.md

```markdown
# {{PROJECT_NAME}}

![Banner]({{PROJECT_SLUG}}-banner.png)

![Version](https://img.shields.io/badge/version-{{VERSION}}-blue)
![License](https://img.shields.io/badge/License-MIT-green)
[![CI](https://github.com/{{GITHUB_ORG}}/{{REPO_NAME}}/workflows/CI/badge.svg)](https://github.com/{{GITHUB_ORG}}/{{REPO_NAME}}/actions)

<!-- Badges: incluye solo los que aplican al proyecto.
     Version → si el proyecto tiene releases.
     License → siempre.
     CI      → solo si hay un workflow de CI configurado.
     MSRV    → solo para proyectos Rust.
     Elimina los que no apliquen. -->

<p align="center">
  <strong>{{TAGLINE_1}} ✦ {{TAGLINE_2}} ✦ {{TAGLINE_3}}</strong><br>
  <em>{{ONE_LINE_DESCRIPTION}}</em>
</p>

<!-- TAGLINE_*: dos o tres atributos del proyecto en formato fragmento.
     Ej: "Offline-first ✦ Zero dependencies ✦ Built for constrained hardware"
     ONE_LINE_DESCRIPTION: una oración. Qué hace. Para qué hardware/contexto si aplica. -->

<p align="center">
  <a href="#about">About</a> ✦
  <a href="#features">Features</a> ✦
  <a href="#installation">Installation</a> ✦
  <a href="#usage">Usage</a> ✦
  <a href="#architecture">Architecture</a> ✦
  <a href="#contributing">Contributing</a>
</p>

<!-- Nav: elimina #architecture si el proyecto no lo requiere. -->

---

## About

**{{PROJECT_NAME}}** {{WHAT_IT_IS}}.

{{WHY_IT_EXISTS}}

{{HOW_IT_WORKS_AT_A_GLANCE}}

### Philosophy

> *"{{PHILOSOPHY_STATEMENT}}"*

{{REALM_ATTRIBUTION_SENTENCE}}
<!-- 🔁 REALM: ej. "This is a CoreRed project, part of the Sxnnyside Project's experimental branch." -->

<!-- WHAT_IT_IS: definición directa. Sin "powerful", sin "seamless".
     WHY_IT_EXISTS: el párrafo que responde "¿por qué no usar otra cosa?".
                    Contexto del problema, no marketing.
     HOW_IT_WORKS_AT_A_GLANCE: opcional. Una o dos oraciones sobre la mecánica
                                central si aporta contexto inmediato.
     PHILOSOPHY_STATEMENT: una línea. La postura del proyecto. -->

## Features

- **{{FEATURE_NAME}}**: {{FEATURE_DESCRIPTION}}
- **{{FEATURE_NAME}}**: {{FEATURE_DESCRIPTION}}
- **{{FEATURE_NAME}}**: {{FEATURE_DESCRIPTION}}

<!-- Lista limpia. Cada item: nombre en bold + descripción de una línea.
     No más de 7-8 items. Si hay más, agrupan o van a la documentación. -->

## Installation

### Prerequisites

- {{PREREQUISITE}} ({{VERSION_CONSTRAINT}})
- {{PREREQUISITE}} ({{VERSION_CONSTRAINT}})

<!-- Elimina esta sección si no hay prerequisitos reales. -->

### From Source

\`\`\`bash
git clone https://github.com/{{GITHUB_ORG}}/{{REPO_NAME}}.git
cd {{REPO_NAME}}

{{INSTALL_COMMANDS}}
\`\`\`

<!-- INSTALL_COMMANDS: los comandos exactos para tener el proyecto corriendo.
     Si hay más de una forma de instalar (cargo install, brew, binario),
     agregar subsecciones adicionales. -->

## Usage

\`\`\`bash
{{USAGE_EXAMPLES}}
\`\`\`

<!-- Muestra los comandos o flujo más comunes.
     No documentes cada flag aquí — eso va en la documentación o --help.
     El objetivo es que alguien pueda arrancar en 60 segundos. -->

## Architecture

\`\`\`
{{REPO_NAME}}/
├── {{DIR_1}}/    # {{DIR_1_DESCRIPTION}}
├── {{DIR_2}}/    # {{DIR_2_DESCRIPTION}}
└── {{DIR_3}}/    # {{DIR_3_DESCRIPTION}}
\`\`\`

<!-- Primer nivel únicamente. Incluye solo los directorios que orientan
     al colaborador nuevo — no el árbol completo.
     Si el proyecto requiere más detalle, crear ARCHITECTURE.md y
     reemplazar esta sección con:

     For a detailed breakdown, see [ARCHITECTURE.md](ARCHITECTURE.md). -->

## Contributing

Contributions are accepted. See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

Before contributing, read the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

This project is licensed under the {{LICENSE_TYPE}} — see the [LICENSE](LICENSE) file for details.

---

<p align="center">
  <strong>{{PROJECT_NAME}}</strong> — {{REALM_ATTRIBUTION}}<br>
  <em>&copy; {{YEAR}} Sxnnyside Project</em>
</p>

<!-- 🔁 REALM: REALM_ATTRIBUTION → "A CoreRed Project" / "An Acid Savior Release" / etc.
     YEAR: año del release inicial. No actualizar cada año. -->
```

---

## CHANGELOG.md

```markdown
# Changelog

All notable changes to **{{PROJECT_NAME}}** are documented here.

This project follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

<!-- Changes staged for the next release go here.
     Move entries to a versioned section when the release ships.
     Delete this section if there's nothing pending. -->

### Added

### Fixed

### Changed

---

## [{{VERSION}}] — {{YYYY-MM-DD}}

<!-- First real release. Duplicate this block for each subsequent version.
     Version format: MAJOR.MINOR.PATCH
     Date format: YYYY-MM-DD, no exceptions. -->

### Added

- {{DESCRIPTION}}

### Changed

- {{DESCRIPTION}}

### Fixed

- {{DESCRIPTION}}

### Removed

- {{DESCRIPTION}}

### Security

- {{DESCRIPTION}}

### Deprecated

- {{DESCRIPTION}}

<!-- Include only the sections that have entries for this release.
     Delete the rest — empty sections add noise. -->

---

[Unreleased]: https://github.com/{{GITHUB_ORG}}/{{REPO_NAME}}/compare/v{{LATEST_VERSION}}...HEAD
[{{VERSION}}]: https://github.com/{{GITHUB_ORG}}/{{REPO_NAME}}/releases/tag/v{{VERSION}}

<!-- Keep the diff links at the bottom updated with each release.
     Format: [VERSION]: URL
     The Unreleased link always points from the latest tag to HEAD. -->
```

---

## CONTRIBUTING.md

```markdown
# Contributing to {{PROJECT_NAME}}

Contributions are welcome — bugs, fixes, features, or documentation.
This document covers how to work with the project as a contributor.

---

## Before You Start

- Search [existing issues](https://github.com/{{GITHUB_ORG}}/{{REPO_NAME}}/issues) before opening a new one.
- For significant changes, open an issue first to discuss the direction before writing code.
- Read the [Code of Conduct](CODE_OF_CONDUCT.md). It applies to all interactions in this project.

---

## Reporting a Bug

Open a [GitHub Issue](https://github.com/{{GITHUB_ORG}}/{{REPO_NAME}}/issues/new/choose) using the bug report template.

Include:
- What you expected to happen
- What actually happened
- Steps to reproduce
- Environment details (OS, runtime version, relevant config)

---

## Proposing a Feature

Open a [GitHub Issue](https://github.com/{{GITHUB_ORG}}/{{REPO_NAME}}/issues/new/choose) using the feature request template, or submit a PR directly if the change is small and self-contained.

For larger features, an issue discussion first avoids wasted effort on both sides.

---

## Workflow

1. Fork the repository and create a branch from `main`.
2. Name your branch descriptively — `fix/crash-on-empty-input`, `feat/offline-mode`.
3. Make your changes.
4. Open a pull request against `main` with a clear description of what changed and why.

---

## Pull Request Checklist

Before submitting:

- [ ] The project builds without errors
- [ ] Changes are described in [CHANGELOG.md](CHANGELOG.md) under `[Unreleased]`
- [ ] The PR description explains what changed and why
- [ ] New behavior is covered by tests where applicable

---

## Commit Style

This project uses [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/). Every commit message must follow the format:

\`\`\`
<type>: <description>

[optional body]
[optional footer]
\`\`\`

Accepted types:

| Type       | Use for                                          |
|------------|--------------------------------------------------|
| `feat`     | New functionality                                |
| `fix`      | Bug fixes                                        |
| `docs`     | Documentation only                               |
| `style`    | Formatting, whitespace — no logic changes        |
| `refactor` | Code restructure without behavior change         |
| `test`     | Adding or updating tests                         |
| `chore`    | Build process, tooling, dependencies             |
| `perf`     | Performance improvements                         |

Examples:

\`\`\`
feat: add offline fallback for config reads
fix: prevent crash when scripts directory is missing
docs: update installation steps for cross-compilation
chore: bump dependencies to latest stable
\`\`\`

Commits that don't follow this format will be flagged during review.

---

## Questions

If something in the codebase is unclear, open an issue with the `question` label before assuming it's a bug.

---

*{{PROJECT_NAME}} is {{REALM_ATTRIBUTION}}. Part of the [Sxnnyside Project](https://sxnnysideproject.com).*
<!-- 🔁 REALM: REALM_ATTRIBUTION -->
```

---

## CODE_OF_CONDUCT.md

```markdown
# Code of Conduct

Sxnnyside Project sees people who want to contribute — not races, genders, sexual identities, or political stances. If you have an idea and want to build something, you're welcome here. Do it correctly.

---

## What This Project Expects

**On code quality**

This project has a standard and it applies to everyone, including maintainers.

- Code must be clean, intentional, and optimized for readability and performance.
- Over-documented code — particularly boilerplate comments generated by AI agents — is not accepted. If a line needs three sentences to explain what it does, the problem is the line.
- AI-assisted contributions are not rejected on principle. AI-generated code that meets the project's quality standard is acceptable. AI-generated code that doesn't is not — regardless of how it was produced.

**On ideas**

There are no bad ideas here, only ideas that aren't ready yet. If a proposal needs more thought, it'll be said directly and without judgment. Take it back, develop it further, and come back when it's sharper.

**On people**

Collaboration here is based on what you bring to the project. Background, identity, and beliefs are yours — they don't factor into whether a contribution is accepted or rejected. What does factor in: the quality of the work and the respect with which you engage.

---

## What Is Not Acceptable

- Submitting low-quality, unoptimized, or AI-generated code that hasn't been reviewed and refined by the contributor.
- Harassment, hostility, or dismissiveness toward any contributor — in issues, PRs, or any project space.
- Treating feedback on your contribution as a personal attack.
- Intentionally wasting maintainer time with contributions that ignore documented guidelines.

---

## Enforcement

Violations are handled directly. A first instance gets a clear explanation of what went wrong and what's expected. Repeated or severe violations result in removal from the project without further discussion.

There is no committee. Decisions are made by the maintainer.

---

## Scope

This Code of Conduct applies to all project spaces: issues, pull requests, commit messages, and any other interaction tied to {{PROJECT_NAME}}.

---

*{{PROJECT_NAME}} is {{REALM_ATTRIBUTION}}. Part of the [Sxnnyside Project](https://sxnnysideproject.com).*
<!-- 🔁 REALM: REALM_ATTRIBUTION -->
```

---

## SECURITY.md

```markdown
# Security

## Reporting a Vulnerability

If you find a security vulnerability in **{{PROJECT_NAME}}**, report it privately before disclosing it publicly.

**Preferred channel:**
Email — `legal.sxnnyside@sxnnysideproject.com`

**Alternative:**
[GitHub Private Security Advisory](https://github.com/{{GITHUB_ORG}}/{{REPO_NAME}}/security/advisories/new)

---

## What to Include

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Your suggested fix, if you have one

The more context you provide, the faster it gets resolved.

---

## Response Time

Expect an initial response within **2–5 calendar days**.

This is a solo-maintained project. That window reflects reality, not indifference.

---

## Process

1. You report privately.
2. The vulnerability is confirmed or dismissed with an explanation.
3. If confirmed, a fix is developed and shipped.
4. You're credited in the changelog unless you prefer otherwise.

Public disclosure is expected after a fix is available. If a fix isn't possible, that will be communicated directly.

---

## Scope

This policy covers the **{{PROJECT_NAME}}** repository only.
For ecosystem-wide security concerns, use the email above.

---

*{{PROJECT_NAME}} is {{REALM_ATTRIBUTION}}. Part of the [Sxnnyside Project](https://sxnnysideproject.com).*
<!-- 🔁 REALM: REALM_ATTRIBUTION -->
```

---

## SUPPORT.md

```markdown
# Support

## Getting Help

**GitHub Issues**
For bugs or unexpected behavior, open an [issue](https://github.com/{{GITHUB_ORG}}/{{REPO_NAME}}/issues). Use the bug report template and include enough context to reproduce the problem.

**Discord**
For questions, general discussion, or anything that isn't a bug — join the Sxnnyside Project community at [discord.gg/{{DISCORD_INVITE}}](https://discord.gg/{{DISCORD_INVITE}}).

---

## Before Asking

- Check the [README](README.md) — installation, usage, and architecture are documented there.
- Search existing [issues](https://github.com/{{GITHUB_ORG}}/{{REPO_NAME}}/issues) before opening a new one.

---

## What This Isn't

This is not a priority support channel. Response times are not guaranteed.
For security vulnerabilities, see [SECURITY.md](SECURITY.md).

---

*{{PROJECT_NAME}} is {{REALM_ATTRIBUTION}}. Part of the [Sxnnyside Project](https://sxnnysideproject.com).*
<!-- 🔁 REALM: REALM_ATTRIBUTION. Nota: esta versión SÍ incluye Discord — la de personal_bundle NO. -->
```

---

## PULL_REQUEST_TEMPLATE.md

```markdown
## What does this PR do?

<!-- One or two sentences. What changed and why. -->

## Type of change

- [ ] `feat` — new functionality
- [ ] `fix` — bug fix
- [ ] `refactor` — code restructure, no behavior change
- [ ] `docs` — documentation only
- [ ] `chore` — build, tooling, dependencies
- [ ] `perf` — performance improvement
- [ ] `style` — formatting, whitespace
- [ ] `test` — adding or updating tests

## Checklist

- [ ] Builds without errors
- [ ] `CHANGELOG.md` updated under `[Unreleased]`
- [ ] Commits follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
- [ ] Tests added or updated where applicable

## Related issue

<!-- Closes #ISSUE_NUMBER — delete if not applicable -->
```

---

## .github/ISSUE_TEMPLATE/bug_report.md

```markdown
---
name: Bug Report
about: Something isn't working as expected
title: "fix: "
labels: bug
assignees: ""
---

## What happened?

<!-- What did you expect? What actually happened? -->

## Steps to reproduce

1.
2.
3.

## Environment

- OS:
- {{PROJECT_NAME}} version:
- {{RUNTIME}}: <!-- e.g. Rust 1.85, Node 20, Flutter 3.x -->

## Additional context

<!-- Logs, screenshots, or anything else that helps. Delete if not applicable. -->
```

---

## .github/ISSUE_TEMPLATE/feature_request.md

```markdown
---
name: Feature Request
about: A proposal for new functionality or an improvement
title: "feat: "
labels: enhancement
assignees: ""
---

## What are you proposing?

<!-- Describe the feature. One paragraph is enough to start. -->

## Why does this belong in {{PROJECT_NAME}}?

<!-- How does it fit the project's scope and philosophy?
     If it's a rough idea that needs more thought, say so — that's fine. -->

## Possible implementation

<!-- Optional. If you have a direction in mind, sketch it here.
     Delete if you don't. -->

## Alternatives considered

<!-- Optional. Other approaches you ruled out and why. Delete if not applicable. -->
```

---

## CODEOWNERS

```
# CODEOWNERS
#
# Every file in this repository is owned by the maintainer.
# Pull requests require review before merging.
#
# Format: <pattern> <owner>
# Docs: https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners

* @{{GITHUB_USER}}
```

---

## LICENSE (MIT)

```
MIT License

Copyright (c) {{YEAR}} {{BRANCH}} by Sxnnyside Project
<!-- 🔁 REALM: si es rama principal, usar "Copyright (c) {{YEAR}} Sxnnyside Project" sin BRANCH -->

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

**Uso recomendado:** cualquier proyecto cuyo valor principal es ser usado, modificado y extendido por la comunidad. El código es el producto — no hay IP, assets ni narrativa que proteger más allá del crédito de autoría.
