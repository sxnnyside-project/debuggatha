//! What detectors read instead of a parser's tree: calls, the functions around them and the loops
//! and tests they sit in, with the same meaning in every language. An adapter fills it from a
//! syntax tree (`treesitter`); `concepts` says what a call means in each language.

mod build;
mod concepts;
mod flow;
mod oxc;
mod treesitter;
mod visibility;

pub use concepts::{Concepts, Meaning, Status, concepts};
pub use flow::{Certainty, Flow};
pub use treesitter::compile_errors;

use crate::syntax::{Backend, FileSyntax};

/// The model of a parsed file, or `None` when the language has no adapter or the file did not
/// parse.
#[must_use]
pub fn model_of(syntax: &FileSyntax, source: &str) -> Option<Model> {
    match syntax.language.backend() {
        Backend::TreeSitter => treesitter::model_of(syntax, source),
        Backend::Oxc => oxc::model_of_js(syntax.language, source),
        Backend::Sfc => oxc::model_of_sfc(syntax.language, source),
    }
}
pub use visibility::{Visibility, is_wired, reads, visibility};

use crate::syntax::Span;

/// A call the code makes, with its arguments and where it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// The whole call, as a byte range.
    pub span: Span,
    /// The callee as written, with `::`, `->`, `?.` and `\` read as `.`, and a `!` after a macro.
    pub callee: String,
    /// The callee, as a byte range.
    pub callee_span: Span,
    /// The arguments including the parentheses.
    pub args: Option<Span>,
    /// Each argument, in order.
    pub arg_list: Vec<Span>,
    /// Arguments built from interpolation or concatenation, as the language writes it.
    pub dynamic_args: Vec<Span>,
    /// Index into `Model::functions` of the innermost function around the call.
    pub function: Option<usize>,
    /// Bindings that the call's arguments read, resolved by name inside the function and the ones
    /// around it.
    pub refs: Vec<usize>,
    /// Loops around the call inside its own function.
    pub loop_depth: u32,
    /// Whether the call is inside test code.
    pub in_test: bool,
}

/// A name introduced by a parameter, a declaration, an assignment or a loop header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The name.
    pub name: String,
    /// Where it is introduced.
    pub span: Span,
    /// Index into `Model::functions` of the function around it.
    pub function: Option<usize>,
    /// The loop body this name iterates, when a loop header introduces it.
    pub loop_body: Option<Span>,
    /// What is assigned to it, when the declaration or assignment shows it.
    pub value: Option<Span>,
}

/// A function, type or method a file defines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    /// The name.
    pub name: String,
    /// Where it is declared.
    pub span: Span,
    /// The line.
    pub line: u32,
    /// Who can see it.
    pub visibility: Visibility,
    /// A member of a class or module, not a declaration of the file itself.
    pub member: bool,
    /// An annotation, decorator or attribute marks it: a framework may use it by that mark.
    pub decorated: bool,
}

/// A name written in the code that is not a call or a declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    /// The name.
    pub name: String,
    /// Where it is.
    pub span: Span,
}

/// What an import statement names, as written (quotes removed), and the line it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    /// What the statement names, as written.
    pub specifier: String,
    /// The line.
    pub line: u32,
}

/// A variable or member chain as written (`request.args`, `$_GET`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    /// Where it is.
    pub span: Span,
    /// The text as written.
    pub text: String,
}

/// What detectors read of a file instead of a parser's tree, with the same meaning in every language.
#[derive(Debug, Default)]
pub struct Model {
    /// Every call.
    pub calls: Vec<Call>,
    /// The functions of the file.
    pub functions: Vec<Span>,
    /// The loops of the file.
    pub loops: Vec<Span>,
    /// The loops that run a fixed small number of times (over a list written out, up to ten
    /// turns): a query in one is not one query per row.
    pub small_loops: Vec<Span>,
    /// Every name introduced in the file.
    pub bindings: Vec<Binding>,
    /// Names read, each with the binding it resolves to.
    pub reads: Vec<(Span, usize)>,
    /// Variable and member chains.
    pub paths: Vec<Path>,
    /// Spans whose value is made safe by what they are (a cast to a number).
    pub cleaned: Vec<Span>,
    /// Import statements.
    pub imports: Vec<Import>,
    /// What the file defines.
    pub declarations: Vec<Declaration>,
    /// Calls whose target is computed at run time: a name search cannot follow them.
    pub dynamic_calls: Vec<Span>,
    /// Error handlers (`catch`, `except`, `rescue`) with nothing in them.
    pub empty_handlers: Vec<Span>,
    /// Every name written in the file that is not the name of a declaration.
    pub mentions: Vec<Mention>,
}

impl Model {
    /// Does the call, from inside a loop, read a name that loop introduces or that is declared in
    /// its body? Any enclosing loop counts: the innermost may iterate over something unrelated.
    #[must_use]
    pub fn reads_iteration(&self, call: &Call) -> bool {
        let floor = call.function.map_or(0, |i| self.functions[i].start);
        self.loops
            .iter()
            .filter(|l| {
                floor <= l.start
                    && l.start <= call.span.start
                    && call.span.end <= l.end
                    && !self.small_loops.contains(l)
            })
            .any(|l| {
                call.refs
                    .iter()
                    .filter_map(|&i| self.bindings.get(i))
                    .any(|b| {
                        b.loop_body == Some(*l) || (l.start <= b.span.start && b.span.end <= l.end)
                    })
            })
    }
}
