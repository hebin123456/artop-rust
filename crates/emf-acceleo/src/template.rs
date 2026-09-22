//! Template model and runtime value context for the MTL engine.
//!
//! A template file (`TemplateFile`) contains a set of template and query
//! declarations. Declarations retain their raw body, which the [`m2t_engine`]
//! interprets at render time against a [`ValueContext`] that binds parameter
//! and local variable names to literal values.

use std::collections::BTreeMap;

/// Visibility modifier of a template or query declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    /// `public`: importable / callable from other files.
    #[default]
    Public,
    /// `private`: only usable within the declaring file.
    Private,
    /// `protected`: usable in the declaring file and sub-files.
    Protected,
}

impl Visibility {
    /// Whether the element may be referenced outside its own file.
    pub fn is_public(self) -> bool {
        self == Visibility::Public
    }
}

/// Declaration kind: a callable `template` or a side-effect-free `query`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclKind {
    /// `[template ...[/template]]`.
    Template,
    /// `[query .../]`.
    Query,
}

/// A single formal parameter of a template or query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateParam {
    /// Parameter name.
    pub name: String,
    /// Declared type, e.g. `EClass` or `::mm::Book`.
    pub ty: String,
}

impl TemplateParam {
    /// Constructor.
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ty: ty.into(),
        }
    }
}

/// A template or query declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateDecl {
    /// Kind of the declaration.
    pub kind: DeclKind,
    /// Simple name.
    pub name: String,
    /// Visibility modifier.
    pub visibility: Visibility,
    /// Formal parameters.
    pub params: Vec<TemplateParam>,
    /// Raw body (for templates; queries keep the expression).
    pub body: String,
    /// One-based line of the declaration header.
    pub line: usize,
}

impl TemplateDecl {
    /// Whether the declaration exposes a public entry point.
    pub fn is_public(&self) -> bool {
        self.visibility.is_public()
    }
}

/// A parsed `.mtl` file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TemplateFile {
    /// All declarations, in source order.
    pub declarations: Vec<TemplateDecl>,
}

impl TemplateFile {
    /// Find a declaration by name.
    pub fn get(&self, name: &str) -> Option<&TemplateDecl> {
        self.declarations.iter().find(|d| d.name == name)
    }

    /// Public declarations with no parameters — suitable as M2T entry points.
    pub fn entry_points(&self) -> Vec<&TemplateDecl> {
        self.declarations
            .iter()
            .filter(|d| d.is_public() && d.params.is_empty())
            .collect()
    }

    /// Push a declaration.
    pub fn push(&mut self, decl: TemplateDecl) {
        self.declarations.push(decl);
    }
}

/// A `[query ...]` alias retained for convenience on top of [`TemplateDecl`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryDecl {
    /// Query name.
    pub name: String,
    /// Parameters.
    pub params: Vec<TemplateParam>,
    /// Expression body.
    pub expression: String,
    /// Declared return type.
    pub return_type: Option<String>,
}

/// Runtime variable bindings used while rendering a template.
#[derive(Debug, Clone, Default)]
pub struct ValueContext {
    /// Bindings from variable name to its string-ified value.
    values: BTreeMap<String, String>,
}

impl ValueContext {
    /// An empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind a variable to a string value.
    pub fn bind(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.values.insert(name.into(), value.into());
        self
    }

    /// Bind a variable to a formatted value.
    pub fn bind_fmt(
        &mut self,
        name: impl Into<String>,
        value: impl std::fmt::Display,
    ) -> &mut Self {
        self.values.insert(name.into(), value.to_string());
        self
    }

    /// Look up a variable.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// Whether a variable is bound.
    pub fn contains(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }

    /// Number of bound variables.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the context is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Clone the underlying bindings.
    pub fn bindings(&self) -> BTreeMap<String, String> {
        self.values.clone()
    }
}

/// Expand a template body by substituting `[name]` references from the context.
pub(crate) fn substitute(body: &str, ctx: &ValueContext) -> String {
    let mut out = String::with_capacity(body.len());
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            // Find the matching closing `]`.
            if let Some(close) = body[i + 1..].find(']') {
                let inner = &body[i + 1..i + 1 + close];
                let token = inner.trim();
                if is_var_ref(token) {
                    let key = &token[1..];
                    if let Some(v) = ctx.get(key) {
                        out.push_str(v);
                    }
                    i += close + 2;
                    continue;
                }
            }
            out.push('[');
            i += 1;
        } else {
            // Copy UTF-8 char boundaries.
            let ch = body[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Whether `token` is a simple `$name` variable reference.
fn is_var_ref(token: &str) -> bool {
    token.starts_with('$')
        && token[1..]
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
}

/// Strip MTL comment blocks `[comment ... /]` and `[/* ... */]`.
pub(crate) fn strip_comments(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(pos) = rest.find("[comment") {
        out.push_str(&rest[..pos]);
        // Look for the closing `]`.
        match rest[pos + 8..].find(']') {
            Some(idx) => {
                rest = &rest[pos + 8 + idx + 1..];
            }
            None => {
                out.push_str(&rest[pos..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_binds_and_looks_up() {
        let mut ctx = ValueContext::new();
        ctx.bind("name", "Alice").bind_fmt("age", 30);
        assert_eq!(ctx.get("name"), Some("Alice"));
        assert_eq!(ctx.get("age"), Some("30"));
        assert!(ctx.contains("name"));
        assert_eq!(ctx.len(), 2);
    }

    #[test]
    fn substitution_replaces_vars() {
        let mut ctx = ValueContext::new();
        ctx.bind("greeting", "Hello");
        let out = substitute("[$greeting], world!", &ctx);
        assert_eq!(out, "Hello, world!");
    }

    #[test]
    fn missing_var_yields_empty() {
        let ctx = ValueContext::new();
        assert_eq!(substitute("[$unknown]", &ctx), "");
    }

    #[test]
    fn comments_stripped() {
        let body = "a[comment ignore this/]b";
        assert_eq!(strip_comments(body), "ab");
    }
}
