//! An index of what the files of a review say about each other: what they export, so a detector
//! can tell that `import { save } from './store'` brings in a function that returns a promise, and
//! the test runner's own configuration (`restoreMocks`).
//!
//! It answers one question and no other: *is this export a function that returns a promise?* A
//! function does when it is `async` or when it declares `: Promise<T>` / `: PromiseLike<T>` (the
//! author wrote the contract down). Anything the index cannot show is "no": an unresolved path, a
//! package, an alias (`@/store`), a file outside the review. A diff review sees only the changed
//! files, so there the index is sparse and says "no" more often; that costs recall, never
//! precision.
//!
//! Resolution is relative (`./x`, `../x`) with the extensions TypeScript tries, `index` files, and
//! the `.js` that names a `.ts` file under ESM. `tsconfig` paths and `package.json` exports are
//! not read.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    BindingPattern, Class, Declaration, ExportDefaultDeclarationKind, Expression, Function,
    FunctionBody, ModuleExportName, ReturnStatement, Statement,
};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_semantic::ScopeFlags;

use crate::detect::input::SourceUnit;
use crate::syntax::oxc::source_type;
use crate::syntax::{Backend, Language};

/// Rules that read the index; building it costs a parse of every JavaScript/TypeScript file, so
/// the runner builds it only when the policy holds one of them.
pub const NEEDS_INDEX: &[&str] = &["no-floating-promises", "leak-mock-restore"];

/// How far a chain of `export { x } from` is followed.
const MAX_HOPS: usize = 8;

const EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];

/// What an import or export names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportName<'n> {
    /// A name the module exports.
    Named(&'n str),
    /// The module's default export.
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Export {
    /// A function that returns a promise.
    Promising,
    /// Anything else the file exports under the name.
    Other,
    /// `export { name } from 'specifier'`, `export { default as x } from ..`.
    Reexport(String, OwnedName),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum OwnedName {
    Named(String),
    Default,
}

#[derive(Debug, Default)]
struct Exports {
    named: BTreeMap<String, Export>,
    default: Option<Export>,
    /// `export * from 'specifier'`
    stars: Vec<String>,
}

impl Exports {
    fn insert(&mut self, exported: &str, entry: Export) {
        if exported == "default" {
            self.default = Some(entry);
        } else {
            self.named.insert(exported.to_owned(), entry);
        }
    }
}

/// What the files of a review export. See the module documentation.
#[derive(Debug, Default)]
pub struct ModuleIndex {
    files: BTreeMap<String, Exports>,
    /// Test runner settings, by the directory of the file that holds them (`""` for the root).
    configs: Vec<(String, TestConfig)>,
}

/// What a test runner's configuration restores on its own, so a test need not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TestConfig {
    /// `restoreMocks: true`: every spy goes back after each test.
    pub restore_mocks: bool,
    /// `unstubGlobals: true` (Vitest).
    pub unstub_globals: bool,
    /// `unstubEnvs: true` (Vitest).
    pub unstub_envs: bool,
}

impl TestConfig {
    const fn or(self, other: Self) -> Self {
        Self {
            restore_mocks: self.restore_mocks || other.restore_mocks,
            unstub_globals: self.unstub_globals || other.unstub_globals,
            unstub_envs: self.unstub_envs || other.unstub_envs,
        }
    }
}

/// Finds `restoreMocks: true` and its kin anywhere in a configuration file. The names are specific
/// to the test runners, so no other key is confused with them.
struct ConfigVisitor(TestConfig);

impl<'a> Visit<'a> for ConfigVisitor {
    fn visit_object_property(&mut self, it: &oxc_ast::ast::ObjectProperty<'a>) {
        if let (oxc_ast::ast::PropertyKey::StaticIdentifier(key), Expression::BooleanLiteral(b)) =
            (&it.key, &it.value)
            && b.value
        {
            match key.name.as_str() {
                "restoreMocks" | "clearMocks" | "resetMocks" | "mockReset" => {
                    self.0.restore_mocks = true;
                }
                "unstubGlobals" => self.0.unstub_globals = true,
                "unstubEnvs" => self.0.unstub_envs = true,
                _ => {}
            }
        }
        oxc_ast_visit::walk::walk_object_property(self, it);
    }
}

const CONFIG_STEMS: &[&str] = &[
    "vitest.config",
    "vitest.workspace",
    "vite.config",
    "jest.config",
];

/// The settings a file holds, if it is a test runner configuration.
fn config_of(file: &str, content: &str) -> Option<TestConfig> {
    let name = file.rsplit('/').next().unwrap_or(file);
    if name == "package.json" {
        let json: serde_json::Value = serde_json::from_str(content).ok()?;
        let jest = json.get("jest")?;
        let on = |key: &str| jest.get(key).and_then(serde_json::Value::as_bool) == Some(true);
        return Some(TestConfig {
            restore_mocks: on("restoreMocks") || on("clearMocks") || on("resetMocks"),
            ..TestConfig::default()
        });
    }
    if !CONFIG_STEMS.iter().any(|s| name.starts_with(s)) {
        return None;
    }
    let language = Language::from_path(std::path::Path::new(file)).filter(|l| is_js_family(*l))?;
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, content, source_type(language, false)).parse();
    if parsed.fatal_error {
        return None;
    }
    let mut visitor = ConfigVisitor(TestConfig::default());
    visitor.visit_program(&parsed.program);
    Some(visitor.0)
}

/// Static methods of `Promise` that return a promise.
pub(in crate::detect) const PROMISE_STATICS: &[&str] = &[
    "resolve",
    "reject",
    "all",
    "allSettled",
    "race",
    "any",
    "withResolvers",
];

/// `new Promise(..)` or `Promise.resolve(..)` and its kin, written out. A local named `Promise` is
/// not looked for here (no scope analysis in the index); it would be a rare shadowing.
pub(in crate::detect) fn is_literal_promise(e: &Expression<'_>) -> bool {
    match super::js::peel(e) {
        Expression::NewExpression(n) => {
            matches!(&n.callee, Expression::Identifier(id) if id.name == "Promise")
        }
        Expression::CallExpression(c) => match &c.callee {
            Expression::StaticMemberExpression(m) => {
                matches!(&m.object, Expression::Identifier(id) if id.name == "Promise")
                    && PROMISE_STATICS.contains(&m.property.name.as_str())
            }
            _ => false,
        },
        _ => false,
    }
}

/// Every `return` of a body (the functions inside it are their own) hands back a literal promise,
/// and there is at least one: the `function sleep(t) { return new Promise(..) }` idiom.
struct Returns {
    count: usize,
    all_promises: bool,
}

impl<'a> Visit<'a> for Returns {
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &oxc_ast::ast::ArrowFunctionExpression<'a>) {}
    fn visit_class(&mut self, _: &Class<'a>) {}
    fn visit_return_statement(&mut self, it: &ReturnStatement<'a>) {
        self.count += 1;
        if !it.argument.as_ref().is_some_and(is_literal_promise) {
            self.all_promises = false;
        }
    }
}

fn only_returns_promises(body: &FunctionBody<'_>) -> bool {
    let mut returns = Returns {
        count: 0,
        all_promises: true,
    };
    returns.visit_function_body(body);
    returns.count > 0 && returns.all_promises
}

/// A function that returns a promise: `async`, one that declares `: Promise<T>` /
/// `: PromiseLike<T>`, or one whose every `return` is a promise written out.
pub(in crate::detect) fn returns_a_promise(f: &Function<'_>) -> bool {
    f.r#async
        || super::functions::declares_a_promise(f.return_type.as_deref())
        || f.body.as_ref().is_some_and(|b| only_returns_promises(b))
}

/// The same for a function expression or an arrow.
pub(in crate::detect) fn expression_returns_a_promise(e: &Expression<'_>) -> bool {
    match super::js::peel(e) {
        Expression::FunctionExpression(f) => returns_a_promise(f),
        Expression::ArrowFunctionExpression(a) => {
            a.r#async
                || super::functions::declares_a_promise(a.return_type.as_deref())
                || a.get_expression().is_some_and(is_literal_promise)
                || a.get_function_body()
                    .is_some_and(|b| only_returns_promises(b))
        }
        _ => false,
    }
}

fn is_js_family(language: Language) -> bool {
    matches!(language.backend(), Backend::Oxc)
}

fn export_of(promising: bool) -> Export {
    if promising {
        Export::Promising
    } else {
        Export::Other
    }
}

fn name_of(name: &ModuleExportName<'_>) -> String {
    name.name().to_string()
}

/// `const f = async () => ..` and its kin: each name a declaration list binds to a function, and
/// whether that function returns a promise.
fn variables_of(v: &oxc_ast::ast::VariableDeclaration<'_>) -> Vec<(String, bool)> {
    v.declarations
        .iter()
        .filter_map(|d| match &d.id {
            BindingPattern::BindingIdentifier(id) => Some((
                id.name.to_string(),
                d.init.as_ref().is_some_and(expression_returns_a_promise),
            )),
            _ => None,
        })
        .collect()
}

/// The top-level functions of a file, by name: `true` if they return a promise.
fn top_level_functions(body: &[Statement<'_>]) -> BTreeMap<String, bool> {
    let mut found = BTreeMap::new();
    for statement in body {
        match statement {
            Statement::FunctionDeclaration(f) => {
                if let Some(id) = &f.id {
                    found.insert(id.name.to_string(), returns_a_promise(f));
                }
            }
            Statement::VariableDeclaration(v) => found.extend(variables_of(v)),
            _ => {}
        }
    }
    found
}

fn exports_of(body: &[Statement<'_>]) -> Exports {
    let local = top_level_functions(body);
    let mut exports = Exports::default();
    for statement in body {
        match statement {
            Statement::ExportDeclaration(e) => match &e.declaration {
                Declaration::FunctionDeclaration(f) => {
                    if let Some(id) = &f.id {
                        exports
                            .named
                            .insert(id.name.to_string(), export_of(returns_a_promise(f)));
                    }
                }
                Declaration::VariableDeclaration(v) => {
                    for (name, promising) in variables_of(v) {
                        exports.named.insert(name, export_of(promising));
                    }
                }
                _ => {}
            },
            Statement::ExportNamedDeclaration(e) => {
                for s in &e.specifiers {
                    let entry = export_of(local.get(&name_of(&s.local)).copied().unwrap_or(false));
                    exports.insert(&name_of(&s.exported), entry);
                }
            }
            Statement::ExportFromDeclaration(e) => {
                for s in &e.specifiers {
                    let imported = name_of(&s.local);
                    let name = if imported == "default" {
                        OwnedName::Default
                    } else {
                        OwnedName::Named(imported)
                    };
                    let entry = Export::Reexport(e.source.value.display().to_string(), name);
                    exports.insert(&name_of(&s.exported), entry);
                }
            }
            Statement::ExportDefaultDeclaration(d) => {
                let promising = match &d.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(f) => returns_a_promise(f),
                    ExportDefaultDeclarationKind::Identifier(id) => {
                        local.get(id.name.as_str()).copied().unwrap_or(false)
                    }
                    other => other
                        .as_expression()
                        .is_some_and(expression_returns_a_promise),
                };
                exports.default = Some(export_of(promising));
            }
            // `export * as ns from` names a namespace, not a function.
            Statement::ExportAllDeclaration(a) if a.exported.is_none() => {
                exports.stars.push(a.source.value.display().to_string());
            }
            _ => {}
        }
    }
    exports
}

/// `dir/a/../b` to `dir/b`; `None` when the path climbs out of the root.
fn normalize(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

impl ModuleIndex {
    /// The index, if a rule of the policy reads it (`NEEDS_INDEX`): it costs a parse of every
    /// JavaScript/TypeScript file, so a policy without such a rule does not pay for it.
    #[must_use]
    pub fn for_policy(
        policy: &debuggatha_core::knowledge_system::ReviewPolicy,
        units: &[SourceUnit],
    ) -> Option<Self> {
        policy
            .rules
            .iter()
            .any(|r| NEEDS_INDEX.contains(&r.id.as_str()))
            .then(|| Self::build(units))
    }

    /// Indexes the JavaScript/TypeScript files among `units`. A file that does not parse at all is
    /// left out.
    #[must_use]
    pub fn build(units: &[SourceUnit]) -> Self {
        let mut files = BTreeMap::new();
        let mut configs = Vec::new();
        for unit in units {
            if let Some(config) = config_of(&unit.file, &unit.content)
                && let Some(path) = normalize(&unit.file)
            {
                let dir = path.rsplit_once('/').map_or("", |(d, _)| d).to_owned();
                configs.push((dir, config));
            }
            let Some(language) = Language::from_path(std::path::Path::new(&unit.file)) else {
                continue;
            };
            if !is_js_family(language) {
                continue;
            }
            let allocator = Allocator::default();
            let parsed =
                Parser::new(&allocator, &unit.content, source_type(language, false)).parse();
            if parsed.fatal_error {
                continue;
            }
            if let Some(key) = normalize(&unit.file) {
                files.insert(key, exports_of(&parsed.program.body));
            }
        }
        Self { files, configs }
    }

    /// What the test runner configurations at or above `file`'s folder restore on their own.
    #[must_use]
    pub fn test_config_for(&self, file: &str) -> TestConfig {
        let path = normalize(file).unwrap_or_default();
        self.configs
            .iter()
            .filter(|(dir, _)| dir.is_empty() || path.starts_with(&format!("{dir}/")))
            .fold(TestConfig::default(), |all, (_, c)| all.or(*c))
    }

    /// The indexed file a `specifier` written in `from` stands for. Only relative specifiers are
    /// resolved.
    #[must_use]
    pub fn resolve(&self, from: &str, specifier: &str) -> Option<String> {
        if !(specifier.starts_with("./") || specifier.starts_with("../")) {
            return None;
        }
        let dir = from.rsplit_once('/').map_or("", |(d, _)| d);
        let base = normalize(&format!("{dir}/{specifier}"))?;
        let mut candidates = vec![base.clone()];
        for ext in EXTENSIONS {
            candidates.push(format!("{base}.{ext}"));
        }
        for ext in EXTENSIONS {
            candidates.push(format!("{base}/index.{ext}"));
        }
        // ESM names the compiled file: `./x.js` is `./x.ts` on disk.
        if let Some((stem, ext)) = base.rsplit_once('.') {
            let swaps: &[&str] = match ext {
                "js" => &["ts", "tsx"],
                "jsx" => &["tsx"],
                "mjs" => &["mts"],
                "cjs" => &["cts"],
                _ => &[],
            };
            for swap in swaps {
                candidates.push(format!("{stem}.{swap}"));
            }
        }
        candidates.into_iter().find(|c| self.files.contains_key(c))
    }

    /// Does `name`, exported by the file `specifier` names from `from`, return a promise?
    #[must_use]
    pub fn export_returns_a_promise(
        &self,
        from: &str,
        specifier: &str,
        name: ExportName<'_>,
    ) -> bool {
        let Some(file) = self.resolve(from, specifier) else {
            return false;
        };
        let owned = match name {
            ExportName::Named(n) => OwnedName::Named(n.to_owned()),
            ExportName::Default => OwnedName::Default,
        };
        self.lookup(&file, &owned, 0, &mut BTreeSet::new())
    }

    fn lookup(
        &self,
        file: &str,
        name: &OwnedName,
        hops: usize,
        seen: &mut BTreeSet<(String, OwnedName)>,
    ) -> bool {
        if hops > MAX_HOPS || !seen.insert((file.to_owned(), name.clone())) {
            return false;
        }
        let Some(exports) = self.files.get(file) else {
            return false;
        };
        let direct = match name {
            OwnedName::Named(n) => exports.named.get(n),
            OwnedName::Default => exports.default.as_ref(),
        };
        match direct {
            Some(Export::Promising) => return true,
            Some(Export::Other) => return false,
            Some(Export::Reexport(specifier, inner)) => {
                return self
                    .resolve(file, specifier)
                    .is_some_and(|next| self.lookup(&next, inner, hops + 1, seen));
            }
            None => {}
        }
        // `export *` never re-exports a default.
        if matches!(name, OwnedName::Default) {
            return false;
        }
        exports.stars.iter().any(|specifier| {
            self.resolve(file, specifier)
                .is_some_and(|next| self.lookup(&next, name, hops + 1, seen))
        })
    }
}
