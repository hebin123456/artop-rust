//! Abstract syntax tree for Acceleo MTL templates (port of C++
//! `emf-acceleo/AcceleoAst.h`, aligned to Java `org.eclipse.acceleo.model.mtpl`
//! and the AQL expression sub-set).
//!
//! The AST splits into two layers:
//!
//! - **Blocks** model the template body: literal text, expression blocks,
//!   `for`/`if`/`let`/`file`/`protected` control blocks.
//! - **Expressions** model the AQL sub-set used inside blocks: variables,
//!   literals, navigation (`c.name`), calls (`->size()`, `name(args)`) and
//!   lambdas (`e | e.name`).
//!
//! Recursive positions are held as `Rc` so the AST is cheaply shareable and the
//! runner can look up templates/queries across a module `extends` chain.

use std::rc::Rc;

/// A formal parameter of a template, query or module: `name : Type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// Parameter name.
    pub name: String,
    /// Declared type name, e.g. `EClass` or `::mm::Book`.
    pub type_name: String,
}

/// An AQL expression node.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A variable reference (`self`, `c`, `x`...).
    Var(String),
    /// A string literal.
    StringLit(String),
    /// An integer literal.
    IntLit(i64),
    /// A boolean literal.
    BoolLit(bool),
    /// Attribute navigation: `expr.name`.
    Nav {
        /// The navigated-from expression.
        target: Rc<Expr>,
        /// The feature name.
        name: String,
    },
    /// A call: `expr->name(args)` (arrow) or `expr.name(args)` / `name(args)`.
    Call {
        /// The call target, if any (`None` for global calls).
        target: Option<Rc<Expr>>,
        /// The call name.
        name: String,
        /// The call arguments.
        args: Vec<Rc<Expr>>,
        /// Whether the `->` arrow form was used.
        arrow: bool,
    },
    /// A collection literal `Collection { e1, e2 }` (rarely used).
    CollectionLit(Vec<Rc<Expr>>),
    /// A conditional expression `cond ? then : else`.
    If {
        /// Condition.
        cond: Rc<Expr>,
        /// Value when truthy.
        then_expr: Rc<Expr>,
        /// Value when falsy.
        else_expr: Rc<Expr>,
    },
    /// A lambda `var | body`, used by `collect`/`select`/`reject`/... .
    Lambda {
        /// The iteration variable.
        var_name: String,
        /// The lambda body.
        body: Rc<Expr>,
    },
}

impl Expr {
    /// Wrap in a fresh `Rc`.
    pub fn rc(self) -> Rc<Expr> {
        Rc::new(self)
    }
}

/// A `[for (v | collection)] ... [/for]` block.
#[derive(Debug, Clone, PartialEq)]
pub struct ForBlock {
    /// Iteration variable name.
    pub var_name: String,
    /// Optional declared element type.
    pub var_type_name: String,
    /// The iterated collection expression.
    pub collection: Rc<Expr>,
    /// Whether a `sep (...)` separator was declared.
    pub has_separator: bool,
    /// The separator text.
    pub separator: String,
    /// The loop body.
    pub body: Vec<Rc<Block>>,
}

/// A `[if (cond)] ... [elseif (c)] ... [else] ... [/if]` block.
#[derive(Debug, Clone, PartialEq)]
pub struct IfBlock {
    /// The `if` condition.
    pub cond: Rc<Expr>,
    /// The `then` body.
    pub then_body: Vec<Rc<Block>>,
    /// The `elseif` branches, in order.
    pub else_ifs: Vec<(Rc<Expr>, Vec<Rc<Block>>)>,
    /// The trailing `else` body (empty when absent).
    pub else_body: Vec<Rc<Block>>,
}

/// A `[let v : T = expr] ... [/let]` block.
#[derive(Debug, Clone, PartialEq)]
pub struct LetBlock {
    /// Variable name.
    pub var_name: String,
    /// Optional declared type.
    pub var_type_name: String,
    /// The bound value expression.
    pub value: Rc<Expr>,
    /// The block body.
    pub body: Vec<Rc<Block>>,
}

/// A `[file (path, append, charset)] ... [/file]` block.
#[derive(Debug, Clone, PartialEq)]
pub struct FileBlock {
    /// The output path expression.
    pub path: Rc<Expr>,
    /// Whether to append instead of overwrite.
    pub append: bool,
    /// Optional charset.
    pub charset: String,
    /// The file body.
    pub body: Vec<Rc<Block>>,
}

/// A `[protected (id)] ... [/protected]` block.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtectedBlock {
    /// Protected-region id (may be empty; the engine defaults it).
    pub id: String,
    /// The block body.
    pub body: Vec<Rc<Block>>,
}

/// A template-body block.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// Literal text, emitted verbatim.
    Text(String),
    /// An expression block `[expr/]`: evaluate and emit the string form.
    Expr(Rc<Expr>),
    /// A `for` block.
    For(ForBlock),
    /// An `if` block.
    If(IfBlock),
    /// A `let` block.
    Let(LetBlock),
    /// A `file` block.
    File(FileBlock),
    /// A `protected` block.
    Protected(ProtectedBlock),
}

impl Block {
    /// Wrap in a fresh `Rc`.
    pub fn rc(self) -> Rc<Block> {
        Rc::new(self)
    }
}

/// A `[template ...] ... [/template]` declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    /// Template name.
    pub name: String,
    /// Formal parameters.
    pub params: Vec<Param>,
    /// Whether the template is public.
    pub is_public: bool,
    /// The template body.
    pub body: Vec<Rc<Block>>,
    /// Optional `[post(...)]` text (retained but unused by the engine).
    pub post_literal: String,
}

/// A `[query ... = expr /]` declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    /// Query name.
    pub name: String,
    /// Formal parameters.
    pub params: Vec<Param>,
    /// Declared return type name.
    pub return_type_name: String,
    /// The query body expression.
    pub body: Rc<Expr>,
}

/// A parsed `[module ...] ... [/module]` file.
#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    /// Module name.
    pub name: String,
    /// Module entry parameters.
    pub params: Vec<Param>,
    /// `[import uri]` entries.
    pub imports: Vec<String>,
    /// `extends` parent module names.
    pub extends: Vec<String>,
    /// Declared templates.
    pub templates: Vec<Rc<Template>>,
    /// Declared queries.
    pub queries: Vec<Rc<Query>>,
}

impl Module {
    /// Find a template by name (own module only).
    pub fn template(&self, name: &str) -> Option<Rc<Template>> {
        self.templates
            .iter()
            .find(|t| t.name == name)
            .map(Rc::clone)
    }

    /// Find a query by name (own module only).
    pub fn query(&self, name: &str) -> Option<Rc<Query>> {
        self.queries.iter().find(|q| q.name == name).map(Rc::clone)
    }
}
