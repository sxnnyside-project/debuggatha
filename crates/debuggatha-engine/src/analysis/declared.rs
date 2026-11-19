//! Dead code and public API for the languages the semantic model reads, from what files declare
//! and which names they mention. By name, across files, and on purpose conservative: a name written
//! anywhere in the scope that can see the declaration counts as a use, so a finding is a name nothing
//! writes. Reflection, string lookups and framework wiring are the ways it can still be wrong,
//! which is why a declaration with an annotation or a conventional entry-point name is left out.
//!
//! - `unreferenced`: a private declaration its file never mentions again, or a package-visible one
//!   no file of its directory mentions.
//! - `exported-but-unused`: a public top-level declaration no other file mentions. Consumers
//!   outside the repository are not measured.

use std::collections::BTreeMap;
use std::path::Path;

use super::boundaries::ModuleBoundary;
use super::dead_code::{Confidence, DeadCodeFinding, DeadCodeKind};
use super::ecosystem::{Ecosystems, descriptors};
use super::exports::ExportForm;
use super::public_api::{PublicApiSurface, PublicApiSymbol, Stability};
use super::workspace::{Workspace, dirname};
use crate::scan_scope::is_test_path;
use crate::semantic::{Model, Visibility, concepts, model_of, reads};
use crate::syntax::{Language, parse};

struct Parsed {
    file: String,
    language: Language,
    model: Model,
}

fn parse_all(ws: &Workspace) -> Vec<Parsed> {
    let mut extensions: Vec<&str> = Vec::new();
    for (_, d) in descriptors() {
        extensions.extend(d.extensions.iter().map(String::as_str));
    }
    ws.files_with(&extensions)
        .into_iter()
        .filter_map(|file| {
            let language = Language::from_path(Path::new(file))?;
            Ecosystems::of(language)?;
            reads(language).then_some(())?;
            let source = ws.read(file)?;
            let model = parse(Path::new(file), source).and_then(|s| model_of(&s, source))?;
            Some(Parsed {
                file: file.to_owned(),
                language,
                model,
            })
        })
        .collect()
}

/// Which files mention each name, by index.
fn mentioned_in(parsed: &[Parsed]) -> BTreeMap<&str, Vec<usize>> {
    let mut by_name: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, p) in parsed.iter().enumerate() {
        for m in &p.model.mentions {
            let files = by_name.entry(m.name.as_str()).or_default();
            if files.last() != Some(&i) {
                files.push(i);
            }
        }
    }
    by_name
}

#[must_use]
pub fn detect_declared_dead_code(ws: &Workspace) -> Vec<DeadCodeFinding> {
    let parsed = parse_all(ws);
    let by_name = mentioned_in(&parsed);
    let table = concepts();
    let mut computed: BTreeMap<Language, usize> = BTreeMap::new();
    for p in &parsed {
        *computed.entry(p.language).or_default() += p.model.dynamic_calls.len();
    }
    let mut out = Vec::new();
    for (i, p) in parsed.iter().enumerate() {
        if is_test_path(&p.file) {
            continue;
        }
        for d in &p.model.declarations {
            if d.decorated || table.is("entry-point", p.language, &d.name) {
                continue;
            }
            let used_in = by_name.get(d.name.as_str()).map_or(&[][..], Vec::as_slice);
            let (kind, confidence, used, rationale) = match d.visibility {
                Visibility::Private => (
                    DeadCodeKind::Unreferenced,
                    Confidence::Medium,
                    used_in.contains(&i),
                    "private to its file and no other mention of the name is in it",
                ),
                Visibility::Module => (
                    DeadCodeKind::Unreferenced,
                    Confidence::Medium,
                    !used_in.is_empty(),
                    "visible inside the repository only, and no file mentions the name",
                ),
                Visibility::Package => (
                    DeadCodeKind::Unreferenced,
                    Confidence::Medium,
                    used_in
                        .iter()
                        .any(|j| dirname(&parsed[*j].file) == dirname(&p.file)),
                    "visible only inside its directory and no file there mentions the name",
                ),
                Visibility::Public if !d.member => (
                    DeadCodeKind::ExportedButUnused,
                    Confidence::Low,
                    used_in.iter().any(|j| *j != i),
                    "public, and no other file of the repository mentions the name; consumers outside it are not measured",
                ),
                Visibility::Public => continue,
            };
            if !used {
                let computed = computed.get(&p.language).copied().unwrap_or(0);
                let followed = kind == DeadCodeKind::Unreferenced && computed > 0;
                out.push(DeadCodeFinding {
                    file: p.file.clone(),
                    symbol: d.name.clone(),
                    line: d.line,
                    kind,
                    confidence: if followed { Confidence::Low } else { confidence },
                    rationale: if followed {
                        format!("{rationale}; the repository calls by a computed name in {computed} place(s), which a name search cannot follow")
                    } else {
                        rationale.to_owned()
                    },
                });
            }
        }
    }
    out
}

#[must_use]
pub fn build_declared_public_api(
    ws: &Workspace,
    boundaries: &[ModuleBoundary],
) -> Vec<PublicApiSurface> {
    let parsed = parse_all(ws);
    boundaries
        .iter()
        .filter_map(|b| {
            let files: Vec<&Parsed> = parsed
                .iter()
                .filter(|p| b.files.contains(&p.file) && !is_test_path(&p.file))
                .collect();
            let language = files.first()?.language;
            let internal = Ecosystems::of(language)?.1.internal_dirs.clone();
            let exported: Vec<PublicApiSymbol> = files
                .iter()
                .filter(|p| {
                    !internal
                        .iter()
                        .any(|d| p.file.split('/').any(|seg| seg == d))
                })
                .flat_map(|p| {
                    p.model
                        .declarations
                        .iter()
                        .filter(|d| d.visibility == Visibility::Public)
                        .map(|d| PublicApiSymbol {
                            name: d.name.clone(),
                            file: p.file.clone(),
                            line: d.line,
                            stability: if d.name.starts_with('_') {
                                Stability::Unstable
                            } else {
                                Stability::Stable
                            },
                            form: ExportForm::Declaration,
                            definition: Some((p.file.clone(), d.name.clone())),
                        })
                })
                .collect();
            Some(PublicApiSurface {
                module_id: b.id.clone(),
                entrypoints: Vec::new(),
                subpaths: Vec::new(),
                exported,
                unexpectedly_exposed: Vec::new(),
                language: language.id(),
                covered: true,
                limits: vec![
                    "the surface is every public declaration of the module's files; re-exports and package-level visibility rules that the language adds are not followed".to_owned(),
                ],
            })
        })
        .collect()
}
