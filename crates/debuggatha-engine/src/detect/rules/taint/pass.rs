//! The pass that walks a file and carries taint from sources to sinks.

use super::ast::{bindings, first_argument, key_name, peel_ts_wrappers, segments};
use super::model::{Certainty, Handler, Imported, Prop, Root, State, best, root_of};
use super::vocabulary::{
    ARGUMENTS_TOO, ELEMENT_CALLBACKS, HONO_METHODS, INJECTION, MONGO_SANITIZERS, PRESERVING,
    SOURCE_PROPS, VALIDATOR_MODULES,
};
use crate::detect::input::DetectorInput;
use crate::detect::rules::js::{Lines, make_hit};
use debuggatha_core::review_engine::{Confidence, Severity};
use oxc_ast::AstKind;
use oxc_ast::ast::{
    ArrayExpressionElement, AssignmentTarget, BinaryOperator, BindingPattern, CallExpression,
    ChainElement, Expression, ForInStatement, ForOfStatement, ForStatementLeft, Function,
    IdentifierReference, ImportDeclaration, ImportDeclarationSpecifier, ObjectPropertyKind,
    StaticMemberExpression, VariableDeclarationKind, VariableDeclarator,
};
use oxc_ast_visit::{Visit, walk};
use oxc_semantic::{ScopeFlags, Semantic, SymbolId};

pub(super) struct Pass<'a, 's, 'x> {
    pub(super) semantic: &'a Semantic<'a>,
    pub(super) input: &'a DetectorInput<'a>,
    pub(super) lines: Lines<'s>,
    pub(super) state: &'x mut State,
    /// First pass: record the structure of the file (handlers, imports, reads).
    pub(super) collect: bool,
    /// Last pass: taint has settled, look for sinks.
    pub(super) report: bool,
}

impl Pass<'_, '_, '_> {
    pub(super) fn symbol(&self, id: &IdentifierReference<'_>) -> Option<SymbolId> {
        self.semantic
            .scoping()
            .get_reference(id.reference_id())
            .symbol_id()
    }

    pub(super) fn root(&self, id: &IdentifierReference<'_>) -> Option<Root> {
        self.symbol(id)
            .and_then(|s| self.state.request.get(&s).copied())
    }

    pub(super) fn reassigned(&self, symbol: SymbolId) -> bool {
        self.semantic
            .scoping()
            .get_resolved_references(symbol)
            .any(oxc_semantic::Reference::is_write)
    }

    pub(super) fn raise(&mut self, symbol: SymbolId, certainty: Certainty) {
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
    pub(super) fn is_request(&self, e: &Expression<'_>) -> bool {
        match peel_ts_wrappers(e) {
            Expression::Identifier(id) => {
                matches!(self.root(id), Some(Root::Req | Root::Ctx))
            }
            Expression::StaticMemberExpression(m) if m.property.name == "request" => {
                matches!(peel_ts_wrappers(&m.object), Expression::Identifier(id) if self.root(id) == Some(Root::Ctx))
            }
            _ => false,
        }
    }

    pub(super) fn is_source_member(&self, m: &StaticMemberExpression<'_>) -> bool {
        SOURCE_PROPS.contains(&m.property.name.as_str()) && self.is_request(&m.object)
    }

    /// `await req.json()`, `c.req.query('id')`.
    pub(super) fn is_source_call(&self, c: &CallExpression<'_>) -> bool {
        let Expression::StaticMemberExpression(m) = peel_ts_wrappers(&c.callee) else {
            return false;
        };
        let method = m.property.name.as_str();
        match peel_ts_wrappers(&m.object) {
            Expression::Identifier(id) => {
                self.root(id) == Some(Root::Req)
                    && ["json", "text", "formData", "param"].contains(&method)
            }
            Expression::StaticMemberExpression(inner) if inner.property.name == "req" => {
                matches!(peel_ts_wrappers(&inner.object), Expression::Identifier(id) if self.root(id) == Some(Root::Hono))
                    && HONO_METHODS.contains(&method)
            }
            _ => false,
        }
    }

    /// Does the expression hold client data, and how sure is that?
    pub(super) fn taint(&self, e: &Expression<'_>) -> Option<Certainty> {
        match peel_ts_wrappers(e) {
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

    pub(super) fn call_taint(&self, c: &CallExpression<'_>) -> Option<Certainty> {
        if self.is_source_call(c) {
            return Some(Certainty::Sure);
        }
        let first = || first_argument(c).and_then(|a| self.taint(a));
        match peel_ts_wrappers(&c.callee) {
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
    pub(super) fn is_whole_object(&self, e: &Expression<'_>) -> bool {
        match peel_ts_wrappers(e) {
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

    pub(super) fn hit(
        &mut self,
        rule: &'static str,
        start: usize,
        end: usize,
        confidence: Confidence,
    ) {
        if let Some(mut hit) = make_hit(self.input, &self.lines, start, end, confidence, None) {
            if confidence == Confidence::Low {
                // A guess about the flow: it must not fail a run the way a proven finding does.
                hit.severity = Some(Severity::Medium);
            }
            self.state.hits.push((rule, hit, start));
        }
    }

    pub(super) fn injection(&mut self, start: usize, end: usize, certainty: Certainty) {
        let confidence = if certainty == Certainty::Sure {
            Confidence::High
        } else {
            Confidence::Low
        };
        self.hit(INJECTION, start, end, confidence);
    }

    /// The module a name was imported or required from, for the two lists above.
    pub(super) fn bind_module(&mut self, module: &str, symbol: SymbolId, member: Option<String>) {
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

    /// Where the function an identifier names starts: `function f(req, res)` or `const f = (req) =>`.
    pub(super) fn function_start(&self, id: &IdentifierReference<'_>) -> Option<usize> {
        let symbol = self.symbol(id)?;
        let node = self.semantic.scoping().symbol_declaration(symbol);
        match self.semantic.nodes().kind(node) {
            AstKind::Function(f) => Some(f.span.start as usize),
            AstKind::VariableDeclarator(d) => match d.init.as_ref().map(peel_ts_wrappers)? {
                Expression::ArrowFunctionExpression(a) => Some(a.span.start as usize),
                Expression::FunctionExpression(f) => Some(f.span.start as usize),
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn enter_function(
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

    pub(super) fn callback_element(&mut self, c: &CallExpression<'_>) {
        let Expression::StaticMemberExpression(m) = peel_ts_wrappers(&c.callee) else {
            return;
        };
        if !ELEMENT_CALLBACKS.contains(&m.property.name.as_str()) {
            return;
        }
        let Some(certainty) = self.taint(&m.object) else {
            return;
        };
        let params = match first_argument(c).map(peel_ts_wrappers) {
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
            && let Expression::Identifier(id) = peel_ts_wrappers(&it.object)
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
    pub(super) fn declare(
        &mut self,
        declarator: &VariableDeclarator<'a>,
        init: &Expression<'a>,
        targets: &[SymbolId],
        sure: bool,
    ) {
        if self.collect
            && let Expression::CallExpression(call) = peel_ts_wrappers(init)
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
            (peel_ts_wrappers(init), &declarator.id)
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
            (peel_ts_wrappers(init), &declarator.id)
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
    pub(super) fn iterate(&mut self, left: &ForStatementLeft<'a>, right: &Expression<'a>) {
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
