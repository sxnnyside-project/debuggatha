//! `a03-injection` and `validate-input`, from one small taint analysis over the request-shaped
//! sources of JavaScript and TypeScript.
//!
//! **Sources**: `req.body|query|params|headers|cookies` where `req` (or `request`) is the first
//! parameter of a function, Koa's `ctx.*`, `await req.json()` and Hono's `c.req.*`. A destructuring
//! taints every name it binds. **Propagation**: inside one file, flow-insensitive, to a fixed point,
//! through bindings, assignments, templates, `+`, `||`/`??`/`?:`, member access, literals, string
//! methods and iteration callbacks. Numeric coercion, anything named like a cleaner and a schema
//! call stop it, and so does a call to a function the file does not define (its result is clean).
//!
//! **`a03-injection` sinks**: SQL text built from tainted data (`query`, `execute`, `raw` on a
//! data-layer or driver receiver, `*Raw`, a template in `where`), the command of `child_process`
//! calls, a `$where` with tainted code and a request object passed whole as a Mongo filter. A
//! tagged `sql` template and a parameterized string are safe. **`validate-input`**: a handler that
//! reads `req.body` or `req.query` and calls no validator, reported high when the file registers it
//! with no validator in the chain and medium when a middleware elsewhere may validate it.
//!
//! **Confidence**: high when the value reaches the sink through `const` bindings only, low (at
//! `medium` severity) when it crosses a variable assigned more than once.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, AssignmentTarget, BinaryOperator, BindingPattern,
    CallExpression, ChainElement, Expression, ForInStatement, ForOfStatement, ForStatementLeft,
    Function, IdentifierName, IdentifierReference, ImportDeclaration, ImportDeclarationSpecifier,
    LogicalOperator, ObjectExpression, ObjectPropertyKind, PropertyKey, StaticMemberExpression,
    UnaryOperator, VariableDeclarationKind, VariableDeclarator,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::{ScopeFlags, Semantic, SemanticBuilder, SymbolId};

use super::calls::one_per_line;
use super::functions::peel_parens;
use super::hit::Hit;
use super::input::DetectorInput;
use super::js::{Lines, make_hit};
use super::runner::Registry;
use crate::scan_scope::is_test_path;
use crate::syntax::Language;
use crate::syntax::oxc::source_type;
use debuggatha_core::review_engine::{Confidence, Severity};

/// Names of a data layer, whole (compared in lower case).
const LAYER_NAMES: &[&str] = &[
    "prisma",
    "db",
    "database",
    "knex",
    "sequelize",
    "sql",
    "pool",
    "connection",
    "conn",
    "trx",
    "tx",
    "transaction",
    "entitymanager",
    "em",
    "manager",
    "queryrunner",
    "repo",
    "repository",
    "orm",
    "datasource",
];

/// Endings that make a name a data layer (`userRepository`, `orderDao`).
const LAYER_SUFFIXES: &[&str] = &["repository", "repo", "dao", "model", "db"];

fn lower(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// The names along a receiver chain, root first: `this.db.user.findMany` is
/// `["this", "db", "user", "findMany"]`; `knex('t').where(..)` is `["knex", "where"]`.
fn segments(e: &Expression<'_>, out: &mut Vec<String>) {
    match peel_parens(e) {
        Expression::Identifier(id) => out.push(id.name.to_string()),
        Expression::StaticMemberExpression(m) => {
            segments(&m.object, out);
            out.push(m.property.name.to_string());
        }
        Expression::ComputedMemberExpression(m) => segments(&m.object, out),
        Expression::CallExpression(c) => segments(&c.callee, out),
        Expression::TSNonNullExpression(x) => segments(&x.expression, out),
        Expression::ChainExpression(c) => {
            if let ChainElement::CallExpression(call) = &c.expression {
                segments(&call.callee, out);
            }
        }
        _ => {}
    }
}

fn is_layer(name: &str) -> bool {
    let l = lower(name);
    LAYER_NAMES.contains(&l.as_str()) || LAYER_SUFFIXES.iter().any(|s| l.ends_with(s))
}

/// `User`, `OrderItem`: a class-like name, not a constant (`API`) or a function.
fn is_class_like(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase) && name.chars().any(char::is_lowercase)
}

/// The rules this detector answers for.
pub const RULES: &[&str] = &["a03-injection", "validate-input"];

const INJECTION: &str = "a03-injection";
const VALIDATE: &str = "validate-input";

/// Registers the detector.
pub fn register(registry: &mut Registry) {
    registry.register_group(RULES, detect);
}

/// Properties of a request that a client controls.
const SOURCE_PROPS: &[&str] = &["body", "query", "params", "headers", "cookies"];

/// Methods of a Hono `c.req` that return client data.
const HONO_METHODS: &[&str] = &[
    "json",
    "text",
    "query",
    "queries",
    "param",
    "header",
    "parseBody",
    "formData",
];

/// Methods whose result still holds what the receiver held.
const PRESERVING: &[&str] = &[
    "trim",
    "trimStart",
    "trimEnd",
    "toString",
    "toLowerCase",
    "toUpperCase",
    "toLocaleLowerCase",
    "toLocaleUpperCase",
    "replace",
    "replaceAll",
    "concat",
    "slice",
    "substring",
    "substr",
    "normalize",
    "padStart",
    "padEnd",
    "join",
    "split",
    "at",
    "filter",
    "flat",
    "find",
    "pop",
    "shift",
    "reverse",
    "sort",
    "valueOf",
    "toJSON",
];

/// Of those, the ones whose arguments end up in the result as well.
const ARGUMENTS_TOO: &[&str] = &[
    "concat",
    "replace",
    "replaceAll",
    "padStart",
    "padEnd",
    "join",
];

/// Methods that call back with each element of the receiver.
const ELEMENT_CALLBACKS: &[&str] = &[
    "forEach",
    "map",
    "filter",
    "flatMap",
    "some",
    "every",
    "find",
    "findIndex",
];

/// Methods of a schema or validator that turn raw data into checked data.
const VALIDATING: &[&str] = &[
    "parse",
    "safeParse",
    "parseAsync",
    "safeParseAsync",
    "validate",
    "validateSync",
    "validateAsync",
    "isValid",
    "isValidSync",
    "cast",
    "sanitize",
];

/// Receivers whose `parse`/`validate` are not a schema's.
const NOT_A_SCHEMA: &[&str] = &[
    "JSON",
    "Number",
    "Date",
    "Math",
    "path",
    "url",
    "URL",
    "qs",
    "querystring",
    "Object",
    "Array",
    "String",
];

/// Modules of validation and sanitizing libraries.
const VALIDATOR_MODULES: &[&str] = &[
    "express-validator",
    "zod",
    "joi",
    "@hapi/joi",
    "yup",
    "ajv",
    "celebrate",
    "class-validator",
    "class-transformer",
    "valibot",
    "superstruct",
    "@sinclair/typebox",
    "express-joi-validation",
    "zod-express-middleware",
    "validator",
];

/// Modules that clean a query object of operators (`$ne`, `$where`).
const MONGO_SANITIZERS: &[&str] = &["express-mongo-sanitize", "mongo-sanitize"];

/// HTTP verbs of a router, besides `use` and `all`.
const VERBS: &[&str] = &[
    "get", "post", "put", "patch", "delete", "head", "options", "all", "use",
];

/// Query methods that need a data-layer receiver (`db.query`, but not `map.get`).
const LAYER_SQL: &[&str] = &[
    "query", "execute", "raw", "prepare", "exec", "run", "all", "get", "each", "literal", "unsafe",
];

/// Methods that take raw SQL whatever the receiver is.
const UNSAFE_BY_NAME: &[&str] = &[
    "$queryRawUnsafe",
    "$executeRawUnsafe",
    "whereRaw",
    "orderByRaw",
    "havingRaw",
    "groupByRaw",
    "joinRaw",
    "fromRaw",
    "selectRaw",
];

/// Query-builder methods that take a condition as text.
const CONDITIONS: &[&str] = &["where", "andWhere", "orWhere", "having", "andHaving"];

/// Database drivers and clients that are not in the layer lists above.
const DRIVERS: &[&str] = &[
    "client", "pg", "mysql", "mysql2", "sqlite", "sqlite3", "postgres", "dbclient", "pgpool",
];

/// Mongo methods that take a filter as their first argument.
const MONGO_METHODS: &[&str] = &[
    "find",
    "findOne",
    "findOneAndUpdate",
    "findOneAndDelete",
    "findOneAndReplace",
    "updateOne",
    "updateMany",
    "deleteOne",
    "deleteMany",
    "countDocuments",
    "aggregate",
];

/// `child_process` functions that run a command line.
const SHELL_FUNCTIONS: &[&str] = &["exec", "execSync"];
/// `child_process` functions whose first argument is the program to run.
const PROGRAM_FUNCTIONS: &[&str] = &["spawn", "spawnSync", "execFile", "execFileSync", "fork"];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Certainty {
    /// Reaches the sink through a variable assigned more than once.
    Maybe,
    /// Every link of the flow is a single binding.
    Sure,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Root {
    /// `req`, `request`.
    Req,
    /// `ctx`, `context`.
    Ctx,
    /// `c`.
    Hono,
}

fn root_of(name: &str) -> Option<Root> {
    match name {
        "req" | "request" => Some(Root::Req),
        "ctx" | "context" => Some(Root::Ctx),
        "c" => Some(Root::Hono),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Prop {
    Body,
    Query,
}

struct Handler {
    start: usize,
    end: usize,
    reads: Vec<(Prop, usize, usize)>,
    /// A validator was called on request data inside.
    validated: bool,
    /// Registered in this file with a route call.
    registered: bool,
    /// Registered behind a validator.
    upstream: bool,
}

/// What a name imported from `child_process` or `shelljs` stands for.
#[derive(Clone)]
enum Imported {
    /// `import cp from`, `import * as cp from`, `const cp = require(..)`.
    Whole,
    Named(String),
}

#[derive(Default)]
struct State {
    request: BTreeMap<SymbolId, Root>,
    handlers: Vec<Handler>,
    handler_of: BTreeMap<SymbolId, usize>,
    tainted: BTreeMap<SymbolId, Certainty>,
    /// Bound to a request object as a whole (`const filter = req.query`).
    whole: BTreeSet<SymbolId>,
    validators: BTreeSet<SymbolId>,
    shell: BTreeMap<SymbolId, Imported>,
    mongo_sanitized: bool,
    /// An `app.use(validate)` somewhere: every route is behind it.
    global_validation: bool,
    /// Spans of logging calls, whose arguments are not a use of the data.
    logging: Vec<(usize, usize)>,
    hits: Vec<(&'static str, Hit, usize)>,
    changed: bool,
}

fn best(items: impl IntoIterator<Item = Option<Certainty>>) -> Option<Certainty> {
    items.into_iter().flatten().max()
}

fn peel<'e, 'a>(mut e: &'e Expression<'a>) -> &'e Expression<'a> {
    loop {
        e = match peel_parens(e) {
            Expression::TSAsExpression(x) => &x.expression,
            Expression::TSSatisfiesExpression(x) => &x.expression,
            Expression::TSNonNullExpression(x) => &x.expression,
            Expression::TSTypeAssertion(x) => &x.expression,
            other => return other,
        };
    }
}

/// The words of a name: `validateUserBody` is `validate`, `user`, `body`; `shell_quote` is `shell`,
/// `quote`. Read by words so that `invalidateCache` does not say it validates.
fn words(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in name.chars() {
        let boundary = !ch.is_alphanumeric()
            || (ch.is_uppercase() && current.chars().last().is_some_and(char::is_lowercase));
        if boundary && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        if ch.is_alphanumeric() {
            current.extend(ch.to_lowercase());
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// A name that says it checks or sanitizes: `validateBody`, `withValidation`, `sanitizeId`.
fn is_validator_name(name: &str) -> bool {
    words(name)
        .iter()
        .any(|w| w.starts_with("valid") || w.starts_with("sanitiz") || w == "celebrate")
}

fn bindings(pattern: &BindingPattern<'_>, out: &mut Vec<SymbolId>) {
    match pattern {
        BindingPattern::BindingIdentifier(id) => out.extend(id.symbol_id.get()),
        BindingPattern::ObjectPattern(o) => {
            for p in &o.properties {
                bindings(&p.value, out);
            }
            if let Some(rest) = &o.rest {
                bindings(&rest.argument, out);
            }
        }
        BindingPattern::ArrayPattern(a) => {
            for element in a.elements.iter().flatten() {
                bindings(element, out);
            }
            if let Some(rest) = &a.rest {
                bindings(&rest.argument, out);
            }
        }
        BindingPattern::AssignmentPattern(a) => bindings(&a.left, out),
    }
}

fn key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(id.name.to_string()),
        PropertyKey::StringLiteral(s) => Some(s.value.display().to_string()),
        _ => None,
    }
}

/// Finds a validator among the names an expression mentions.
struct FindValidator<'a, 'b> {
    semantic: &'a Semantic<'a>,
    validators: &'b BTreeSet<SymbolId>,
    found: bool,
}

impl<'a> Visit<'a> for FindValidator<'a, '_> {
    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        let symbol = self
            .semantic
            .scoping()
            .get_reference(it.reference_id())
            .symbol_id();
        if is_validator_name(&it.name) || symbol.is_some_and(|s| self.validators.contains(&s)) {
            self.found = true;
        }
    }

    fn visit_identifier_name(&mut self, it: &IdentifierName<'a>) {
        if is_validator_name(&it.name) {
            self.found = true;
        }
    }

    // A function among the arguments is the handler or a middleware, not a validator: what is
    // inside it says nothing about what stands before it.
    fn visit_function(&mut self, _it: &Function<'a>, _flags: ScopeFlags) {}

    fn visit_arrow_function_expression(&mut self, _it: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
    }
}

struct Pass<'a, 's, 'x> {
    semantic: &'a Semantic<'a>,
    input: &'a DetectorInput<'a>,
    lines: Lines<'s>,
    state: &'x mut State,
    /// First pass: record the structure of the file (handlers, imports, reads).
    collect: bool,
    /// Last pass: taint has settled, look for sinks.
    report: bool,
}

impl<'a> Pass<'a, '_, '_> {
    fn symbol(&self, id: &IdentifierReference<'_>) -> Option<SymbolId> {
        self.semantic
            .scoping()
            .get_reference(id.reference_id())
            .symbol_id()
    }

    fn root(&self, id: &IdentifierReference<'_>) -> Option<Root> {
        self.symbol(id)
            .and_then(|s| self.state.request.get(&s).copied())
    }

    fn reassigned(&self, symbol: SymbolId) -> bool {
        self.semantic
            .scoping()
            .get_resolved_references(symbol)
            .any(oxc_semantic::Reference::is_write)
    }

    fn raise(&mut self, symbol: SymbolId, certainty: Certainty) {
        if self
            .state
            .tainted
            .get(&symbol)
            .is_none_or(|old| *old < certainty)
        {
            self.state.tainted.insert(symbol, certainty);
            self.state.changed = true;
        }
    }

    /// `req.body`, `ctx.query`, `ctx.request.body`: the request object itself.
    fn is_request(&self, e: &Expression<'_>) -> bool {
        match peel(e) {
            Expression::Identifier(id) => {
                matches!(self.root(id), Some(Root::Req | Root::Ctx))
            }
            Expression::StaticMemberExpression(m) if m.property.name == "request" => {
                matches!(peel(&m.object), Expression::Identifier(id) if self.root(id) == Some(Root::Ctx))
            }
            _ => false,
        }
    }

    fn is_source_member(&self, m: &StaticMemberExpression<'_>) -> bool {
        SOURCE_PROPS.contains(&m.property.name.as_str()) && self.is_request(&m.object)
    }

    /// `await req.json()`, `c.req.query('id')`.
    fn is_source_call(&self, c: &CallExpression<'_>) -> bool {
        let Expression::StaticMemberExpression(m) = peel(&c.callee) else {
            return false;
        };
        let method = m.property.name.as_str();
        match peel(&m.object) {
            Expression::Identifier(id) => {
                self.root(id) == Some(Root::Req)
                    && ["json", "text", "formData", "param"].contains(&method)
            }
            Expression::StaticMemberExpression(inner) if inner.property.name == "req" => {
                matches!(peel(&inner.object), Expression::Identifier(id) if self.root(id) == Some(Root::Hono))
                    && HONO_METHODS.contains(&method)
            }
            _ => false,
        }
    }

    /// Does the expression hold client data, and how sure is that?
    fn taint(&self, e: &Expression<'_>) -> Option<Certainty> {
        match peel(e) {
            Expression::Identifier(id) => self
                .symbol(id)
                .and_then(|s| self.state.tainted.get(&s).copied()),
            Expression::StaticMemberExpression(m) => {
                if self.is_source_member(m) {
                    Some(Certainty::Sure)
                } else {
                    self.taint(&m.object)
                }
            }
            Expression::ComputedMemberExpression(m) => self.taint(&m.object),
            Expression::CallExpression(c) => self.call_taint(c),
            Expression::AwaitExpression(a) => self.taint(&a.argument),
            Expression::ChainExpression(c) => match &c.expression {
                ChainElement::CallExpression(call) => self.call_taint(call),
                ChainElement::StaticMemberExpression(m) => {
                    if self.is_source_member(m) {
                        Some(Certainty::Sure)
                    } else {
                        self.taint(&m.object)
                    }
                }
                ChainElement::ComputedMemberExpression(m) => self.taint(&m.object),
                _ => None,
            },
            Expression::TemplateLiteral(t) => best(t.expressions.iter().map(|x| self.taint(x))),
            Expression::BinaryExpression(b) if b.operator == BinaryOperator::Addition => {
                best([self.taint(&b.left), self.taint(&b.right)])
            }
            Expression::LogicalExpression(l) => best([self.taint(&l.left), self.taint(&l.right)]),
            Expression::ConditionalExpression(c) => {
                best([self.taint(&c.consequent), self.taint(&c.alternate)])
            }
            Expression::SequenceExpression(s) => s.expressions.last().and_then(|x| self.taint(x)),
            Expression::AssignmentExpression(a) => self.taint(&a.right),
            Expression::ArrayExpression(a) => best(a.elements.iter().map(|el| match el {
                ArrayExpressionElement::SpreadElement(s) => self.taint(&s.argument),
                other => other.as_expression().and_then(|x| self.taint(x)),
            })),
            Expression::ObjectExpression(o) => best(o.properties.iter().map(|p| match p {
                ObjectPropertyKind::ObjectProperty(p) => self.taint(&p.value),
                ObjectPropertyKind::SpreadProperty(s) => self.taint(&s.argument),
            })),
            _ => None,
        }
    }

    fn call_taint(&self, c: &CallExpression<'_>) -> Option<Certainty> {
        if self.is_source_call(c) {
            return Some(Certainty::Sure);
        }
        let first = || first_argument(c).and_then(|a| self.taint(a));
        match peel(&c.callee) {
            Expression::Identifier(id) => match id.name.as_str() {
                "String" | "decodeURIComponent" | "decodeURI" | "unescape" => first(),
                _ => None,
            },
            Expression::StaticMemberExpression(m) => {
                let method = m.property.name.as_str();
                let mut parts = Vec::new();
                segments(&m.object, &mut parts);
                let root = parts.first().map_or("", String::as_str);
                if root == "JSON"
                    || matches!(root, "qs" | "querystring")
                    || (root == "Object"
                        && ["values", "entries", "fromEntries", "keys"].contains(&method))
                    || (root == "Array" && method == "from")
                {
                    return first();
                }
                if PRESERVING.contains(&method) {
                    let mut candidates = vec![self.taint(&m.object)];
                    if ARGUMENTS_TOO.contains(&method) {
                        candidates.extend(
                            c.arguments
                                .iter()
                                .map(|a| a.as_expression().and_then(|x| self.taint(x))),
                        );
                    }
                    return best(candidates);
                }
                None
            }
            _ => None,
        }
    }

    /// `req.body` or `req.query` itself, as opposed to a field of it.
    fn is_whole_object(&self, e: &Expression<'_>) -> bool {
        match peel(e) {
            Expression::StaticMemberExpression(m) => self.is_source_member(m),
            Expression::Identifier(id) => self
                .symbol(id)
                .is_some_and(|s| self.state.whole.contains(&s)),
            Expression::ObjectExpression(o) => o.properties.iter().any(|p| {
                matches!(p, ObjectPropertyKind::SpreadProperty(s) if self.is_whole_object(&s.argument))
            }),
            _ => false,
        }
    }

    fn hit(&mut self, rule: &'static str, start: usize, end: usize, confidence: Confidence) {
        if let Some(mut hit) = make_hit(self.input, &self.lines, start, end, confidence, None) {
            if confidence == Confidence::Low {
                // A guess about the flow: it must not fail a run the way a proven finding does.
                hit.severity = Some(Severity::Medium);
            }
            self.state.hits.push((rule, hit, start));
        }
    }

    fn injection(&mut self, start: usize, end: usize, certainty: Certainty) {
        let confidence = if certainty == Certainty::Sure {
            Confidence::High
        } else {
            Confidence::Low
        };
        self.hit(INJECTION, start, end, confidence);
    }

    /// The module a name was imported or required from, for the two lists above.
    fn bind_module(&mut self, module: &str, symbol: SymbolId, member: Option<String>) {
        let module = module.strip_prefix("node:").unwrap_or(module);
        if VALIDATOR_MODULES.contains(&module) {
            self.state.validators.insert(symbol);
        }
        if MONGO_SANITIZERS.contains(&module) {
            self.state.mongo_sanitized = true;
        }
        if matches!(module, "child_process" | "shelljs") {
            self.state
                .shell
                .insert(symbol, member.map_or(Imported::Whole, Imported::Named));
        }
    }

    /// Is the call one that runs the shell, and what does it run?
    fn shell_sink(&self, c: &CallExpression<'_>) -> Option<Certainty> {
        let function = match peel(&c.callee) {
            Expression::Identifier(id) => {
                match self.symbol(id).and_then(|s| self.state.shell.get(&s)) {
                    Some(Imported::Named(name)) => name.clone(),
                    _ => return None,
                }
            }
            Expression::StaticMemberExpression(m) => match peel(&m.object) {
                Expression::Identifier(id)
                    if matches!(
                        self.symbol(id).and_then(|s| self.state.shell.get(&s)),
                        Some(Imported::Whole)
                    ) =>
                {
                    m.property.name.to_string()
                }
                _ => return None,
            },
            _ => return None,
        };
        let function = function.as_str();
        let command = first_argument(c).and_then(|a| self.taint(a));
        if SHELL_FUNCTIONS.contains(&function) || PROGRAM_FUNCTIONS.contains(&function) {
            if command.is_some() {
                return command;
            }
            // `spawn('ls', [x], { shell: true })`: the arguments go through a shell.
            if PROGRAM_FUNCTIONS.contains(&function) && has_shell_option(c) {
                return c
                    .arguments
                    .get(1)
                    .and_then(Argument::as_expression)
                    .and_then(|a| self.taint(a));
            }
        }
        None
    }

    /// SQL text built from client data.
    fn sql_sink(&self, c: &CallExpression<'_>) -> Option<Certainty> {
        let Expression::StaticMemberExpression(m) = peel(&c.callee) else {
            return None;
        };
        let method = m.property.name.as_str();
        let first = first_argument(c)?;
        if UNSAFE_BY_NAME.contains(&method) {
            return self.taint(first);
        }
        if CONDITIONS.contains(&method) {
            return match peel(first) {
                Expression::TemplateLiteral(_) | Expression::BinaryExpression(_) => {
                    self.taint(first)
                }
                _ => None,
            };
        }
        if LAYER_SQL.contains(&method) {
            let mut parts = Vec::new();
            segments(&m.object, &mut parts);
            if parts
                .iter()
                .any(|r| is_layer(r) || DRIVERS.contains(&lower(r).as_str()))
            {
                return self.taint(first);
            }
        }
        None
    }

    /// `User.find(req.query)`: the client picks the operators.
    fn mongo_sink(&self, c: &CallExpression<'_>) -> bool {
        if self.state.mongo_sanitized {
            return false;
        }
        let Expression::StaticMemberExpression(m) = peel(&c.callee) else {
            return false;
        };
        if !MONGO_METHODS.contains(&m.property.name.as_str()) {
            return false;
        }
        let mut parts = Vec::new();
        segments(&m.object, &mut parts);
        let receiver = parts.first().is_some_and(|r| is_class_like(r))
            || parts
                .iter()
                .any(|r| is_layer(r) || lower(r).contains("collection"));
        receiver && first_argument(c).is_some_and(|a| self.is_whole_object(a))
    }

    fn is_validator_call(&self, c: &CallExpression<'_>) -> bool {
        let mut finder = FindValidator {
            semantic: self.semantic,
            validators: &self.state.validators,
            found: false,
        };
        match peel(&c.callee) {
            Expression::StaticMemberExpression(m) => {
                let mut parts = Vec::new();
                segments(&m.object, &mut parts);
                let root = parts.first().map_or("", String::as_str);
                if VALIDATING.contains(&m.property.name.as_str()) && !NOT_A_SCHEMA.contains(&root) {
                    return true;
                }
                finder.visit_expression(&c.callee);
            }
            callee => finder.visit_expression(callee),
        }
        finder.found
    }

    /// Is the expression, or a name inside it, a validator?
    fn mentions_validator(&self, e: &Expression<'a>) -> bool {
        let mut finder = FindValidator {
            semantic: self.semantic,
            validators: &self.state.validators,
            found: false,
        };
        finder.visit_expression(e);
        finder.found
    }

    /// Marks the handlers a route call registers, and whether a validator stands before them.
    fn registration(&mut self, c: &CallExpression<'a>) {
        let Expression::StaticMemberExpression(m) = peel(&c.callee) else {
            return;
        };
        let verb = m.property.name.as_str();
        if !VERBS.contains(&verb) {
            return;
        }
        let expressions: Vec<&Expression<'a>> = c
            .arguments
            .iter()
            .filter_map(Argument::as_expression)
            .collect();
        let validators: Vec<bool> = expressions
            .iter()
            .map(|e| self.mentions_validator(e))
            .collect();
        if verb == "use" && validators.iter().any(|v| *v) {
            self.state.global_validation = true;
        }
        for (i, e) in expressions.iter().enumerate() {
            let (start, end) = (
                oxc_span::GetSpan::span(*e).start as usize,
                oxc_span::GetSpan::span(*e).end as usize,
            );
            let mut starts: Vec<usize> = Vec::new();
            if let Expression::Identifier(id) = peel(e) {
                if let Some(s) = self.function_start(id) {
                    starts.push(s);
                }
            } else {
                starts.extend(
                    self.state
                        .handlers
                        .iter()
                        .filter(|h| h.start >= start && h.end <= end)
                        .map(|h| h.start),
                );
            }
            let upstream = validators[..i].iter().any(|v| *v)
                || matches!(peel(e), Expression::CallExpression(call) if self.is_validator_call(call));
            for h in &mut self.state.handlers {
                if starts.contains(&h.start) {
                    h.registered = true;
                    h.upstream |= upstream;
                }
            }
        }
    }

    /// Where the function an identifier names starts: `function f(req, res)` or `const f = (req) =>`.
    fn function_start(&self, id: &IdentifierReference<'_>) -> Option<usize> {
        let symbol = self.symbol(id)?;
        let node = self.semantic.scoping().symbol_declaration(symbol);
        match self.semantic.nodes().kind(node) {
            AstKind::Function(f) => Some(f.span.start as usize),
            AstKind::VariableDeclarator(d) => match d.init.as_ref().map(peel)? {
                Expression::ArrowFunctionExpression(a) => Some(a.span.start as usize),
                Expression::FunctionExpression(f) => Some(f.span.start as usize),
                _ => None,
            },
            _ => None,
        }
    }

    fn enter_function(
        &mut self,
        params: &oxc_ast::ast::FormalParameters<'_>,
        start: u32,
        end: u32,
    ) {
        if !self.collect {
            return;
        }
        let Some(first) = params.items.first() else {
            return;
        };
        let BindingPattern::BindingIdentifier(id) = &first.pattern else {
            return;
        };
        let (Some(symbol), Some(root)) = (id.symbol_id.get(), root_of(&id.name)) else {
            return;
        };
        self.state.request.insert(symbol, root);
        // A parameter named `request` is often a Fetch `Request`, whose `body` is a stream; it is a
        // handler's only when the next parameter is the response.
        let responds = params.items.get(1).is_some_and(|p| {
            matches!(&p.pattern, BindingPattern::BindingIdentifier(r)
                if matches!(r.name.as_str(), "res" | "response" | "reply" | "_res" | "_reply"))
        });
        if root == Root::Req && (id.name == "req" || responds) {
            self.state
                .handler_of
                .insert(symbol, self.state.handlers.len());
            self.state.handlers.push(Handler {
                start: start as usize,
                end: end as usize,
                reads: Vec::new(),
                validated: false,
                registered: false,
                upstream: false,
            });
        }
    }

    /// `!req.body`, `typeof req.body`, `if (req.body)`, `req.body && ..`: asking whether the data
    /// is there is not trusting it.
    fn is_presence_check(&self, it: &StaticMemberExpression<'_>) -> bool {
        let span = it.span;
        match self.semantic.nodes().parent_kind(it.node_id.get()) {
            AstKind::UnaryExpression(u) => {
                matches!(
                    u.operator,
                    UnaryOperator::LogicalNot | UnaryOperator::Typeof
                )
            }
            AstKind::IfStatement(i) => oxc_span::GetSpan::span(&i.test) == span,
            AstKind::LogicalExpression(l) => {
                l.operator != LogicalOperator::Coalesce && oxc_span::GetSpan::span(&l.left) == span
            }
            _ => false,
        }
    }

    fn callback_element(&mut self, c: &CallExpression<'_>) {
        let Expression::StaticMemberExpression(m) = peel(&c.callee) else {
            return;
        };
        if !ELEMENT_CALLBACKS.contains(&m.property.name.as_str()) {
            return;
        }
        let Some(certainty) = self.taint(&m.object) else {
            return;
        };
        let params = match first_argument(c).map(peel) {
            Some(Expression::ArrowFunctionExpression(a)) => &a.params,
            Some(Expression::FunctionExpression(f)) => &f.params,
            _ => return,
        };
        let mut symbols = Vec::new();
        if let Some(first) = params.items.first() {
            bindings(&first.pattern, &mut symbols);
        }
        for s in symbols {
            self.raise(s, certainty);
        }
    }

    fn sinks(&mut self, c: &CallExpression<'a>) {
        let (start, end) = (c.span.start as usize, c.span.end as usize);
        if let Some(certainty) = self.sql_sink(c).or_else(|| self.shell_sink(c)) {
            self.injection(start, end, certainty);
        } else if self.mongo_sink(c) {
            self.hit(INJECTION, start, end, Confidence::Low);
        }
        // `validator.parse(req.body)`: the handler validates what it reads.
        if self.is_validator_call(c) {
            let takes_request = c
                .arguments
                .iter()
                .filter_map(Argument::as_expression)
                .any(|a| {
                    self.taint(a).is_some()
                        || matches!(peel(a), Expression::Identifier(id) if self.root(id).is_some())
                });
            if takes_request {
                for h in &mut self.state.handlers {
                    if h.start <= start && end <= h.end {
                        h.validated = true;
                    }
                }
            }
        }
        self.registration(c);
    }
}

fn first_argument<'c, 'b>(c: &'c CallExpression<'b>) -> Option<&'c Expression<'b>> {
    c.arguments.first().and_then(Argument::as_expression)
}

fn has_shell_option(c: &CallExpression<'_>) -> bool {
    c.arguments
        .iter()
        .filter_map(Argument::as_expression)
        .any(|a| match peel(a) {
            Expression::ObjectExpression(o) => option_is_true(o, "shell"),
            _ => false,
        })
}

fn option_is_true(o: &ObjectExpression<'_>, name: &str) -> bool {
    o.properties.iter().any(|p| match p {
        ObjectPropertyKind::ObjectProperty(p) => {
            key_name(&p.key).as_deref() == Some(name)
                && matches!(&p.value, Expression::BooleanLiteral(b) if b.value)
        }
        ObjectPropertyKind::SpreadProperty(_) => false,
    })
}

impl<'a> Visit<'a> for Pass<'a, '_, '_> {
    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        self.enter_function(&it.params, it.span.start, it.span.end);
        walk::walk_function(self, it, flags);
    }

    fn visit_arrow_function_expression(&mut self, it: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
        self.enter_function(&it.params, it.span.start, it.span.end);
        walk::walk_arrow_function_expression(self, it);
    }

    fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
        if self.collect
            && let Expression::Identifier(id) = peel(&it.object)
            && let Some(handler) = self
                .symbol(id)
                .and_then(|s| self.state.handler_of.get(&s).copied())
        {
            let prop = match it.property.name.as_str() {
                "body" => Some(Prop::Body),
                "query" => Some(Prop::Query),
                _ => None,
            };
            if let Some(prop) = prop
                && !self.is_presence_check(it)
            {
                self.state.handlers[handler].reads.push((
                    prop,
                    it.span.start as usize,
                    it.span.end as usize,
                ));
            }
        }
        walk::walk_static_member_expression(self, it);
    }

    fn visit_variable_declaration(&mut self, it: &oxc_ast::ast::VariableDeclaration<'a>) {
        for declarator in &it.declarations {
            if let Some(init) = &declarator.init {
                let mut targets = Vec::new();
                bindings(&declarator.id, &mut targets);
                let sure = it.kind == VariableDeclarationKind::Const
                    || !targets.iter().any(|t| self.reassigned(*t));
                self.declare(declarator, init, &targets, sure);
            }
        }
        walk::walk_variable_declaration(self, it);
    }

    fn visit_assignment_expression(&mut self, it: &oxc_ast::ast::AssignmentExpression<'a>) {
        if let AssignmentTarget::AssignmentTargetIdentifier(id) = &it.left
            && let Some(symbol) = self.symbol(id)
            && self.taint(&it.right).is_some()
        {
            self.raise(symbol, Certainty::Maybe);
        }
        walk::walk_assignment_expression(self, it);
    }

    fn visit_for_of_statement(&mut self, it: &ForOfStatement<'a>) {
        self.iterate(&it.left, &it.right);
        walk::walk_for_of_statement(self, it);
    }

    fn visit_for_in_statement(&mut self, it: &ForInStatement<'a>) {
        self.iterate(&it.left, &it.right);
        walk::walk_for_in_statement(self, it);
    }

    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
        if self.collect {
            let module = it.source.value.display().to_string();
            for specifier in it.specifiers.iter().flatten() {
                match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(s) => {
                        if let Some(symbol) = s.local.symbol_id.get() {
                            self.bind_module(&module, symbol, Some(s.imported.name().to_string()));
                        }
                    }
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                        if let Some(symbol) = s.local.symbol_id.get() {
                            self.bind_module(&module, symbol, None);
                        }
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                        if let Some(symbol) = s.local.symbol_id.get() {
                            self.bind_module(&module, symbol, None);
                        }
                    }
                }
            }
        }
        walk::walk_import_declaration(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if self.collect {
            let mut parts = Vec::new();
            segments(&it.callee, &mut parts);
            if parts
                .first()
                .is_some_and(|r| matches!(r.as_str(), "console" | "logger" | "log"))
            {
                self.state
                    .logging
                    .push((it.span.start as usize, it.span.end as usize));
            }
        }
        self.callback_element(it);
        if self.report {
            self.sinks(it);
        }
        walk::walk_call_expression(self, it);
    }

    fn visit_object_property(&mut self, it: &oxc_ast::ast::ObjectProperty<'a>) {
        if self.report
            && key_name(&it.key).as_deref() == Some("$where")
            && let Some(certainty) = self.taint(&it.value)
        {
            self.injection(it.span.start as usize, it.span.end as usize, certainty);
        }
        walk::walk_object_property(self, it);
    }
}

impl<'a> Pass<'a, '_, '_> {
    /// `const x = init`: taint the names, and read `require('child_process')`.
    fn declare(
        &mut self,
        declarator: &VariableDeclarator<'a>,
        init: &Expression<'a>,
        targets: &[SymbolId],
        sure: bool,
    ) {
        if self.collect
            && let Expression::CallExpression(call) = peel(init)
            && let Expression::Identifier(callee) = &call.callee
            && callee.name == "require"
            && let Some(Expression::StringLiteral(module)) = first_argument(call)
        {
            let module = module.value.display().to_string();
            match &declarator.id {
                BindingPattern::BindingIdentifier(id) => {
                    if let Some(symbol) = id.symbol_id.get() {
                        self.bind_module(&module, symbol, None);
                    }
                }
                BindingPattern::ObjectPattern(o) => {
                    for p in &o.properties {
                        let (Some(name), BindingPattern::BindingIdentifier(id)) =
                            (key_name(&p.key), &p.value)
                        else {
                            continue;
                        };
                        if let Some(symbol) = id.symbol_id.get() {
                            self.bind_module(&module, symbol, Some(name));
                        }
                    }
                }
                _ => {}
            }
        }
        // `const { body, query } = req`
        if let (Expression::Identifier(id), BindingPattern::ObjectPattern(pattern)) =
            (peel(init), &declarator.id)
            && matches!(self.root(id), Some(Root::Req | Root::Ctx))
        {
            for p in &pattern.properties {
                let Some(name) = key_name(&p.key) else {
                    continue;
                };
                if !SOURCE_PROPS.contains(&name.as_str()) {
                    continue;
                }
                let mut symbols = Vec::new();
                bindings(&p.value, &mut symbols);
                for s in symbols {
                    self.raise(
                        s,
                        if sure {
                            Certainty::Sure
                        } else {
                            Certainty::Maybe
                        },
                    );
                }
                if self.collect
                    && let Some(handler) = self
                        .symbol(id)
                        .and_then(|s| self.state.handler_of.get(&s).copied())
                {
                    let prop = match name.as_str() {
                        "body" => Some(Prop::Body),
                        "query" => Some(Prop::Query),
                        _ => None,
                    };
                    if let Some(prop) = prop {
                        self.state.handlers[handler].reads.push((
                            prop,
                            p.span.start as usize,
                            p.span.end as usize,
                        ));
                    }
                }
            }
            return;
        }
        if let (Expression::StaticMemberExpression(m), BindingPattern::BindingIdentifier(id)) =
            (peel(init), &declarator.id)
            && self.is_source_member(m)
            && let Some(symbol) = id.symbol_id.get()
        {
            self.state.whole.insert(symbol);
        }
        if let Some(certainty) = self.taint(init) {
            for t in targets {
                self.raise(*t, if sure { certainty } else { Certainty::Maybe });
            }
        }
    }

    /// `for (const x of tainted)`, `for (const k in tainted)`.
    fn iterate(&mut self, left: &ForStatementLeft<'a>, right: &Expression<'a>) {
        let ForStatementLeft::VariableDeclaration(declaration) = left else {
            return;
        };
        let Some(certainty) = self.taint(right) else {
            return;
        };
        let mut symbols = Vec::new();
        for d in &declaration.declarations {
            bindings(&d.id, &mut symbols);
        }
        for s in symbols {
            self.raise(s, certainty);
        }
    }
}

/// Runs the detector over one file.
#[must_use]
pub fn detect(input: &DetectorInput<'_>) -> Vec<(&'static str, Hit)> {
    let Some(syntax) = input.syntax else {
        return Vec::new();
    };
    // A request is named `req`, `request`, `ctx`, `context` or `c`: a file with none of the first
    // four cannot hold one (a speed check, not a detection).
    if is_test_path(input.file)
        || !matches!(
            syntax.language,
            Language::JavaScript | Language::Jsx | Language::TypeScript | Language::Tsx
        )
        || !["req", "ctx", "context"]
            .iter()
            .any(|n| input.content.contains(n))
    {
        return Vec::new();
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(
        &allocator,
        input.content,
        source_type(syntax.language, false),
    )
    .parse();
    if parsed.fatal_error {
        return Vec::new();
    }
    let semantic = SemanticBuilder::new()
        .with_build_nodes(true)
        .build(&parsed.program)
        .semantic;
    let mut state = State::default();
    let run = |state: &mut State, collect: bool, report: bool| {
        Pass {
            semantic: &semantic,
            input,
            lines: Lines::new(input.content),
            state,
            collect,
            report,
        }
        .visit_program(&parsed.program);
    };
    // Taint only grows, so the passes end; the cap is for a file built to make them long.
    for pass in 0..16 {
        state.changed = false;
        run(&mut state, pass == 0, false);
        if !state.changed {
            break;
        }
    }
    run(&mut state, false, true);
    handler_hits(input, &mut state);
    one_per_line(std::mem::take(&mut state.hits))
}

/// The `validate-input` hits: handlers that read the body or the query and validate nothing.
fn handler_hits(input: &DetectorInput<'_>, state: &mut State) {
    let lines = Lines::new(input.content);
    let mut found = Vec::new();
    for handler in &state.handlers {
        if handler.validated || handler.upstream || state.global_validation {
            continue;
        }
        let confidence = if handler.registered {
            Confidence::High
        } else {
            Confidence::Medium
        };
        for prop in [Prop::Body, Prop::Query] {
            let first = handler
                .reads
                .iter()
                .filter(|(p, start, end)| {
                    *p == prop
                        && !state
                            .logging
                            .iter()
                            .any(|(from, to)| *from <= *start && *end <= *to)
                })
                .min_by_key(|(_, start, _)| *start);
            if let Some((_, start, end)) = first
                && let Some(hit) = make_hit(input, &lines, *start, *end, confidence, None)
            {
                found.push((VALIDATE, hit, *start));
            }
        }
    }
    state.hits.extend(found);
}
