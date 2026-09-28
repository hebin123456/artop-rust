//! Recursive-descent parser for the Xcore DSL.
//!
//! Port of C++ `emf-ecore/xcore/XcoreParser.cpp` (aligned to Java
//! `org.eclipse.emf.ecore.xcore.resource.XcoreResource`'s parser part). The
//! grammar covered:
//!
//! - `//` line and `/* */` block comments.
//! - `annotation "uri" as Name` directives.
//! - `@Directive` / `@Directive(k=v, k2="str")` annotations.
//! - `package qualified.name { decl* }` (braces optional — Xcore's canonical
//!   form is brace-less, with top-level declarations following the package).
//! - `[abstract|interface] class Name [extends A, B] { member* }`.
//! - members: attributes / `contains`/`refers` references / `op` operations.
//! - member modifiers: `final|readonly|volatile|transient|unsettable|derived|id|unique|resolve`.
//! - `Type[multi]? name [= default] [opposite Name] [get { body }]`.
//! - `op ReturnType name(params) [throws E1, E2] { body }`.
//! - `enum Name { LIT [= v], ... }`.
//! - `type Name wraps qualified.TypeName`.

use crate::dsl::{
    Annotation, AnnotationDirective, AttributeDecl, ClassDecl, DataTypeDecl, EnumDecl,
    EnumLiteralDecl, OperationDecl, PackageDecl, ParameterDecl, ReferenceDecl, ReferenceKind,
};

/// A single error produced while parsing Xcore source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// One-based line of the offending token.
    pub line: usize,
    /// One-based column of the offending token.
    pub column: usize,
    /// Human-readable message.
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "line {} col {}: {}",
            self.line, self.column, self.message
        )
    }
}

impl std::error::Error for ParseError {}

/// The result of parsing a complete Xcore file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedFile {
    /// The top-level package declaration, if present.
    pub package: Option<PackageDecl>,
}

impl ParsedFile {
    /// Convenience accessor for the package class list.
    pub fn classes(&self) -> &[ClassDecl] {
        self.package.as_ref().map_or(&[], |p| &p.classes)
    }
}

/// Attribute/reference modifier keywords (C++ `kAttrModifiers`).
const ATTR_MODIFIERS: &[&str] = &[
    "final",
    "readonly",
    "volatile",
    "transient",
    "unsettable",
    "derived",
    "id",
    "unique",
    "resolve",
];

/// A parsed modifier chain preceding a class member (C++ `MemberMods`).
#[derive(Debug, Clone, Default)]
struct MemberMods {
    annotations: Vec<Annotation>,
    read_only: bool,
    volatile: bool,
    transient: bool,
    unsettable: bool,
    derived: bool,
    id: bool,
    resolve: bool,
}

/// Which kind of member a token sequence declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemberKind {
    Attribute,
    Reference,
    Operation,
}

fn is_ident_part(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

type PResult<T> = Result<T, ParseError>;

impl Parser {
    fn new(src: &str) -> Self {
        Self {
            chars: src.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn err_at(&self, pos: usize, message: impl Into<String>) -> ParseError {
        let mut line = 1usize;
        let mut column = 1usize;
        for c in self.chars.iter().take(pos.min(self.chars.len())) {
            if *c == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        ParseError {
            line,
            column,
            message: message.into(),
        }
    }

    fn err(&self, message: impl Into<String>) -> ParseError {
        self.err_at(self.pos, message)
    }

    fn skip_ws_and_comments(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.pos += 1;
                continue;
            }
            if c == '/' && self.chars.get(self.pos + 1) == Some(&'/') {
                self.pos += 2;
                while let Some(c) = self.peek() {
                    self.pos += 1;
                    if c == '\n' {
                        break;
                    }
                }
                continue;
            }
            if c == '/' && self.chars.get(self.pos + 1) == Some(&'*') {
                self.pos += 2;
                while self.pos + 1 < self.chars.len()
                    && !(self.chars[self.pos] == '*' && self.chars[self.pos + 1] == '/')
                {
                    self.pos += 1;
                }
                if self.pos + 1 < self.chars.len() {
                    self.pos += 2;
                }
                continue;
            }
            break;
        }
    }

    fn peek_char(&mut self, c: char) -> bool {
        self.skip_ws_and_comments();
        self.peek() == Some(c)
    }

    fn consume_char(&mut self, c: char) -> bool {
        if self.peek_char(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect_char(&mut self, c: char, what: &str) -> PResult<()> {
        if self.consume_char(c) {
            Ok(())
        } else {
            Err(self.err(format!("expected '{c}' {what}")))
        }
    }

    /// Match `kw`, requiring the following char to be a non-identifier char.
    fn match_keyword(&mut self, kw: &str) -> bool {
        self.skip_ws_and_comments();
        let n = kw.chars().count();
        if self.pos + n > self.chars.len() {
            return false;
        }
        let matched = kw
            .chars()
            .enumerate()
            .all(|(i, kc)| self.chars[self.pos + i] == kc);
        if !matched {
            return false;
        }
        let after = self.pos + n;
        if let Some(c) = self.chars.get(after) {
            if is_ident_part(*c) {
                return false;
            }
        }
        true
    }

    fn consume_keyword(&mut self, kw: &str) -> bool {
        if self.match_keyword(kw) {
            self.pos += kw.chars().count();
            true
        } else {
            false
        }
    }

    fn parse_identifier(&mut self) -> PResult<String> {
        self.skip_ws_and_comments();
        let start = self.pos;
        while let Some(c) = self.peek() {
            if is_ident_part(c) {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(self.err("expected identifier"));
        }
        Ok(self.chars[start..self.pos].iter().collect())
    }

    fn parse_qualified_name(&mut self) -> PResult<String> {
        let mut name = self.parse_identifier()?;
        while self.peek_char('.') {
            self.pos += 1;
            name.push('.');
            name.push_str(&self.parse_identifier()?);
        }
        Ok(name)
    }

    fn parse_string_literal(&mut self) -> PResult<String> {
        self.skip_ws_and_comments();
        self.expect_char('"', "for string literal")?;
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if c == '"' {
                break;
            }
            self.pos += 1;
            if c == '\\' {
                if let Some(e) = self.peek() {
                    self.pos += 1;
                    match e {
                        'n' => out.push('\n'),
                        't' => out.push('\t'),
                        'r' => out.push('\r'),
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        other => {
                            out.push('\\');
                            out.push(other);
                        }
                    }
                }
            } else {
                out.push(c);
            }
        }
        self.expect_char('"', "to close string literal")?;
        Ok(out)
    }

    fn parse_integer(&mut self) -> PResult<i64> {
        self.skip_ws_and_comments();
        let start = self.pos;
        if matches!(self.peek(), Some('-') | Some('+')) {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(self.err("expected integer"));
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        text.parse::<i64>()
            .map_err(|_| self.err_at(start, format!("invalid integer `{text}`")))
    }

    // ===== annotations =====

    /// Parse zero or more `@Directive` / `@Directive(k=v, ...)`.
    fn parse_annotations(&mut self) -> PResult<Vec<Annotation>> {
        let mut out = Vec::new();
        while self.peek_char('@') {
            self.pos += 1; // consume '@'
            let mut a = Annotation::new(self.parse_identifier()?);
            if self.peek_char('(') {
                self.pos += 1; // consume '('
                self.skip_ws_and_comments();
                while !self.peek_char(')') {
                    let key = self.parse_identifier()?;
                    self.expect_char('=', "in annotation detail")?;
                    self.skip_ws_and_comments();
                    let val = if self.peek_char('"') {
                        self.parse_string_literal()?
                    } else {
                        let start = self.pos;
                        while let Some(c) = self.peek() {
                            if c == ',' || c == ')' || c.is_whitespace() {
                                break;
                            }
                            self.pos += 1;
                        }
                        self.chars[start..self.pos].iter().collect()
                    };
                    a.details.push((key, val));
                    self.skip_ws_and_comments();
                    if self.peek_char(',') {
                        self.pos += 1;
                        self.skip_ws_and_comments();
                    }
                }
                self.expect_char(')', "to close annotation details")?;
            }
            out.push(a);
            self.skip_ws_and_comments();
        }
        Ok(out)
    }

    /// Parse `annotation "uri" as Name` directives into `pkg`.
    fn parse_annotation_directives(&mut self, pkg: &mut PackageDecl) -> PResult<()> {
        while self.match_keyword("annotation") {
            self.consume_keyword("annotation");
            let source_uri = self.parse_string_literal()?;
            if !self.consume_keyword("as") {
                return Err(self.err("expected 'as' after annotation URI"));
            }
            let name = self.parse_identifier()?;
            pkg.annotation_directives
                .push(AnnotationDirective { name, source_uri });
        }
        Ok(())
    }

    // ===== package =====

    fn parse_package(&mut self) -> PResult<PackageDecl> {
        let mut pkg = PackageDecl::default();

        // Package-level annotations before `package` (e.g. `@Ecore(nsURI=...)`).
        let pkg_annots = self.parse_annotations()?;
        for a in pkg_annots {
            if a.directive_name == "Ecore" {
                for (k, v) in &a.details {
                    if k == "nsURI" {
                        pkg.ns_uri = v.clone();
                    } else if k == "nsPrefix" {
                        pkg.ns_prefix = v.clone();
                    }
                }
            }
            pkg.annotations.push(a);
        }

        if !self.consume_keyword("package") {
            return Err(self.err("expected 'package'"));
        }
        pkg.name = self.parse_qualified_name()?;

        // Directives directly after the package header.
        self.parse_annotation_directives(&mut pkg)?;

        // Body: either brace-delimited or running to EOF / next `package`.
        if self.peek_char('{') {
            self.pos += 1; // consume '{'
            self.parse_package_body(&mut pkg, true)?;
        } else {
            self.parse_package_body(&mut pkg, false)?;
        }

        // Defaults (align to Java XcorePackageManager).
        if pkg.ns_prefix.is_empty() {
            pkg.ns_prefix = match pkg.name.rsplit_once('.') {
                Some((_, last)) => last.to_string(),
                None => pkg.name.clone(),
            };
        }
        if pkg.ns_uri.is_empty() {
            pkg.ns_uri = format!("http://{}", pkg.name);
        }

        Ok(pkg)
    }

    /// Parse the package body. When `braced`, stop at the closing `}`; otherwise
    /// stop at EOF or the next `package` keyword.
    fn parse_package_body(&mut self, pkg: &mut PackageDecl, braced: bool) -> PResult<()> {
        self.skip_ws_and_comments();
        loop {
            if braced {
                if self.peek_char('}') {
                    self.pos += 1;
                    break;
                }
            } else if self.pos >= self.chars.len() || self.match_keyword("package") {
                break;
            }
            if self.pos >= self.chars.len() {
                if braced {
                    return Err(self.err("unexpected end of input, expected '}'"));
                }
                break;
            }

            if self.match_keyword("annotation") {
                self.parse_annotation_directives(pkg)?;
                continue;
            }
            if self.consume_keyword("import") {
                self.parse_qualified_name()?;
                if self.consume_keyword("as") {
                    self.parse_identifier()?;
                }
                continue;
            }

            let annots = self.parse_annotations()?;
            if self.match_keyword("class")
                || self.match_keyword("abstract")
                || self.match_keyword("interface")
            {
                let mut cls = self.parse_class()?;
                cls.annotations = annots;
                pkg.classes.push(cls);
            } else if self.match_keyword("enum") {
                let mut e = self.parse_enum()?;
                e.annotations = annots;
                pkg.enums.push(e);
            } else if self.match_keyword("type") {
                let mut dt = self.parse_data_type()?;
                dt.annotations = annots;
                pkg.data_types.push(dt);
            } else {
                return Err(self.err("expected class/enum/type in package body"));
            }
            self.skip_ws_and_comments();
        }
        Ok(())
    }

    fn parse_class(&mut self) -> PResult<ClassDecl> {
        let mut cls = ClassDecl::default();
        if self.consume_keyword("abstract") {
            cls.is_abstract = true;
            if !self.consume_keyword("class") {
                return Err(self.err("expected 'class' after 'abstract'"));
            }
        } else if self.consume_keyword("interface") {
            cls.is_interface = true;
            if !self.consume_keyword("class") {
                return Err(self.err("expected 'class' after 'interface'"));
            }
        } else if !self.consume_keyword("class") {
            return Err(self.err("expected 'class'"));
        }
        cls.name = self.parse_identifier()?;
        if self.consume_keyword("extends") {
            cls.super_types.push(self.parse_qualified_name()?);
            while self.consume_char(',') {
                cls.super_types.push(self.parse_qualified_name()?);
            }
        }
        self.expect_char('{', "to open class body")?;
        self.parse_class_body(&mut cls)?;
        self.expect_char('}', "to close class body")?;
        Ok(cls)
    }

    fn parse_enum(&mut self) -> PResult<EnumDecl> {
        if !self.consume_keyword("enum") {
            return Err(self.err("expected 'enum'"));
        }
        let mut e = EnumDecl {
            name: self.parse_identifier()?,
            ..Default::default()
        };
        self.expect_char('{', "to open enum body")?;
        self.skip_ws_and_comments();
        let mut next_val = 0i32;
        while !self.peek_char('}') {
            let lit_annots = self.parse_annotations()?;
            let name = self.parse_identifier()?;
            let (value, next) = if self.consume_char('=') {
                let v = self.parse_integer()? as i32;
                (Some(v), v + 1)
            } else {
                (Some(next_val), next_val + 1)
            };
            e.literals.push(EnumLiteralDecl {
                name: name.clone(),
                value,
                literal: name,
                annotations: lit_annots,
            });
            next_val = next;
            self.skip_ws_and_comments();
            if self.peek_char(',') {
                self.pos += 1;
                self.skip_ws_and_comments();
            }
        }
        self.expect_char('}', "to close enum body")?;
        Ok(e)
    }

    fn parse_data_type(&mut self) -> PResult<DataTypeDecl> {
        if !self.consume_keyword("type") {
            return Err(self.err("expected 'type'"));
        }
        let name = self.parse_identifier()?;
        if !self.consume_keyword("wraps") {
            return Err(self.err("expected 'wraps' in type declaration"));
        }
        let wrapped_class_name = self.parse_qualified_name()?;
        Ok(DataTypeDecl {
            name,
            wrapped_class_name,
            ..Default::default()
        })
    }

    fn parse_class_body(&mut self, cls: &mut ClassDecl) -> PResult<()> {
        self.skip_ws_and_comments();
        while !self.peek_char('}') {
            if self.pos >= self.chars.len() {
                return Err(self.err("unexpected end of input in class body"));
            }
            let mods = self.parse_member_mods()?;
            match self.classify_member() {
                MemberKind::Operation => {
                    let op = self.parse_operation(&mods)?;
                    cls.operations.push(op);
                }
                MemberKind::Reference => {
                    let r = self.parse_reference(&mods)?;
                    cls.references.push(r);
                }
                MemberKind::Attribute => {
                    let a = self.parse_attribute(&mods)?;
                    cls.attributes.push(a);
                }
            }
            self.skip_ws_and_comments();
            // Members may be separated by an optional comma.
            if self.peek_char(',') {
                self.pos += 1;
                self.skip_ws_and_comments();
            }
        }
        Ok(())
    }

    /// Determine the member kind at the current position without consuming.
    fn classify_member(&mut self) -> MemberKind {
        let save = self.pos;
        let is_op = self.match_keyword("op");
        let is_ref = self.match_keyword("contains") || self.match_keyword("refers");
        self.pos = save;
        if is_op {
            MemberKind::Operation
        } else if is_ref {
            MemberKind::Reference
        } else {
            MemberKind::Attribute
        }
    }

    fn parse_member_mods(&mut self) -> PResult<MemberMods> {
        let mut m = MemberMods {
            resolve: true,
            ..Default::default()
        };
        m.annotations = self.parse_annotations()?;
        loop {
            let mut matched = false;
            for kw in ATTR_MODIFIERS {
                if self.match_keyword(kw) {
                    self.consume_keyword(kw);
                    matched = true;
                    match *kw {
                        "readonly" => m.read_only = true,
                        "volatile" => m.volatile = true,
                        "transient" => m.transient = true,
                        "unsettable" => m.unsettable = true,
                        "derived" => m.derived = true,
                        "id" => m.id = true,
                        "resolve" => {
                            self.skip_ws_and_comments();
                            // `resolve false` disables; `resolve true` / bare
                            // `resolve` keep the default (`true`).
                            let negated = self.consume_keyword("false");
                            if !negated {
                                self.consume_keyword("true");
                            }
                            m.resolve = !negated;
                        }
                        _ => {}
                    }
                    break;
                }
            }
            if !matched {
                break;
            }
        }
        Ok(m)
    }

    fn parse_attribute(&mut self, mods: &MemberMods) -> PResult<AttributeDecl> {
        let mut attr = AttributeDecl {
            annotations: mods.annotations.clone(),
            type_name: self.parse_qualified_name()?,
            ..Default::default()
        };
        attr.multi = self.parse_multiplicity()?;
        attr.name = self.parse_identifier()?;
        attr.read_only = mods.read_only;
        attr.volatile = mods.volatile;
        attr.transient = mods.transient;
        attr.unsettable = mods.unsettable;
        attr.derived = mods.derived;
        attr.id = mods.id;
        if self.consume_char('=') {
            self.skip_ws_and_comments();
            attr.default_value_literal = Some(if self.peek_char('"') {
                self.parse_string_literal()?
            } else {
                let start = self.pos;
                while let Some(c) = self.peek() {
                    if c.is_whitespace() || c == ',' || c == '}' {
                        break;
                    }
                    self.pos += 1;
                }
                self.chars[start..self.pos].iter().collect()
            });
        }
        self.skip_ws_and_comments();
        if self.match_keyword("get") {
            self.consume_keyword("get");
            attr.getter_body = Some(self.parse_brace_body()?);
            attr.derived = true;
        }
        Ok(attr)
    }

    fn parse_reference(&mut self, mods: &MemberMods) -> PResult<ReferenceDecl> {
        let mut r = ReferenceDecl {
            annotations: mods.annotations.clone(),
            read_only: mods.read_only,
            volatile: mods.volatile,
            transient: mods.transient,
            unsettable: mods.unsettable,
            derived: mods.derived,
            resolve_proxies: mods.resolve,
            ..Default::default()
        };
        if self.consume_keyword("contains") {
            r.kind = ReferenceKind::Containment;
        } else if self.consume_keyword("refers") {
            r.kind = ReferenceKind::NonContainment;
        } else {
            return Err(self.err("expected 'contains' or 'refers'"));
        }
        r.type_name = self.parse_qualified_name()?;
        r.multi = self.parse_multiplicity()?;
        r.name = self.parse_identifier()?;
        if self.consume_keyword("opposite") {
            r.opposite_name = Some(self.parse_identifier()?);
        }
        self.skip_ws_and_comments();
        if self.match_keyword("get") {
            self.consume_keyword("get");
            r.getter_body = Some(self.parse_brace_body()?);
            r.derived = true;
        }
        Ok(r)
    }

    fn parse_operation(&mut self, mods: &MemberMods) -> PResult<OperationDecl> {
        if !self.consume_keyword("op") {
            return Err(self.err("expected 'op'"));
        }
        let mut op = OperationDecl {
            annotations: mods.annotations.clone(),
            type_name: self.parse_qualified_name()?,
            ..Default::default()
        };
        op.name = self.parse_identifier()?;
        self.expect_char('(', "to open op params")?;
        self.skip_ws_and_comments();
        while !self.peek_char(')') {
            let type_name = self.parse_qualified_name()?;
            let name = self.parse_identifier()?;
            if self.peek_char('[') {
                self.pos += 1;
                self.expect_char(']', "in param multiplicity")?;
            }
            op.parameters.push(ParameterDecl { name, type_name });
            self.skip_ws_and_comments();
            if self.peek_char(',') {
                self.pos += 1;
                self.skip_ws_and_comments();
            }
        }
        self.expect_char(')', "to close op params")?;
        if self.consume_keyword("throws") {
            op.exceptions.push(self.parse_qualified_name()?);
            while self.consume_char(',') {
                op.exceptions.push(self.parse_qualified_name()?);
            }
        }
        if self.peek_char('{') {
            op.body = Some(self.parse_brace_body()?);
        }
        Ok(op)
    }

    /// Parse a multiplicity suffix `[...]`. Returns `true` for an unbounded
    /// (many) upper bound (C++ `parseMultiplicity`).
    fn parse_multiplicity(&mut self) -> PResult<bool> {
        if !self.peek_char('[') {
            return Ok(false);
        }
        self.pos += 1; // consume '['
        self.skip_ws_and_comments();
        let mut multi = false;
        if self.peek_char('*') || self.peek_char(']') {
            multi = true;
        } else {
            let lower = self.parse_integer()?;
            self.skip_ws_and_comments();
            if self.peek_char('.') {
                self.pos += 1;
                if self.peek_char('.') {
                    self.pos += 1;
                }
                self.skip_ws_and_comments();
                if self.peek_char('*') {
                    multi = true;
                    self.pos += 1;
                } else {
                    let upper = self.parse_integer()?;
                    multi = lower != upper;
                }
            }
        }
        while self.pos < self.chars.len() && self.chars[self.pos] != ']' {
            self.pos += 1;
        }
        self.expect_char(']', "to close multiplicity")?;
        Ok(multi)
    }

    /// Parse `{ ... }`, returning the inner text verbatim (C++ `parseBraceBody`).
    fn parse_brace_body(&mut self) -> PResult<String> {
        self.expect_char('{', "to open brace body")?;
        let mut depth = 1i32;
        let mut body = String::new();
        while self.pos < self.chars.len() && depth > 0 {
            let c = self.chars[self.pos];
            self.pos += 1;
            match c {
                '{' => {
                    depth += 1;
                    body.push(c);
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    body.push(c);
                }
                '"' => {
                    body.push(c);
                    while self.pos < self.chars.len() && self.chars[self.pos] != '"' {
                        body.push(self.chars[self.pos]);
                        self.pos += 1;
                    }
                    if self.pos < self.chars.len() {
                        body.push(self.chars[self.pos]);
                        self.pos += 1;
                    }
                }
                _ => body.push(c),
            }
        }
        Ok(body)
    }
}

/// Parse Xcore source text into a [`ParsedFile`].
///
/// Aligned to C++ `XcoreParser::parse`, which always delegates to
/// `parsePackage`. A file's first meaningful token is either a package-level
/// annotation (`@Ecore(nsURI=...)`) or the `package` keyword itself.
pub fn parse(src: &str) -> Result<ParsedFile, ParseError> {
    let mut p = Parser::new(src);
    p.skip_ws_and_comments();
    if p.match_keyword("package") || p.peek_char('@') {
        let pkg = p.parse_package()?;
        p.skip_ws_and_comments();
        if p.pos != p.chars.len() {
            return Err(p.err("unexpected trailing content after package"));
        }
        Ok(ParsedFile { package: Some(pkg) })
    } else {
        Ok(ParsedFile { package: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_class_with_members() {
        let src = r#"
            package demo
            class Foo {
                String name
                int count
                contains Bar[] bars
                op String greet() { "hi" }
            }
            class Bar {
                boolean active
            }
        "#;
        let file = parse(src).expect("parse ok");
        let pkg = file.package.unwrap();
        assert_eq!(pkg.name, "demo");
        assert_eq!(pkg.classes.len(), 2);
        let foo = pkg.class("Foo").unwrap();
        assert_eq!(foo.attributes.len(), 2);
        assert_eq!(foo.references.len(), 1);
        assert_eq!(foo.operations.len(), 1);
        assert_eq!(foo.references[0].type_name, "Bar");
        assert_eq!(foo.references[0].kind, ReferenceKind::Containment);
        assert!(foo.references[0].multi);
        assert_eq!(foo.operations[0].name, "greet");
        assert_eq!(foo.operations[0].type_name, "String");
    }

    #[test]
    fn parses_braced_package() {
        let src = "package demo { class Foo { String name } }";
        let pkg = parse(src).unwrap().package.unwrap();
        assert_eq!(pkg.name, "demo");
        assert_eq!(pkg.classes.len(), 1);
        assert_eq!(pkg.class("Foo").unwrap().attributes.len(), 1);
    }

    #[test]
    fn parses_extends_and_enum_auto_increment() {
        let src = r#"
            package example
            class Base { String id }
            class Derived extends Base { refers Base parent }
            enum Color { RED = 0, GREEN, BLUE }
        "#;
        let pkg = parse(src).unwrap().package.unwrap();
        assert_eq!(pkg.classes.len(), 2);
        assert_eq!(pkg.classes[1].super_types, vec!["Base".to_string()]);
        assert_eq!(pkg.enums.len(), 1);
        let e = &pkg.enums[0];
        assert_eq!(e.literals.len(), 3);
        assert_eq!(e.literals[0].value, Some(0));
        assert_eq!(e.literals[1].value, Some(1));
        assert_eq!(e.literals[2].value, Some(2));
    }

    #[test]
    fn parses_annotations_and_modifiers() {
        let src = r#"
            @Ecore(nsURI="http://test", nsPrefix="t")
            package test
            annotation "http://www.eclipse.org/emf/2002/Ecore" as Ecore
            class Node {
                @Ecore(name="NODE")
                derived long average get { 0 }
                readonly String label
                id String uuid
            }
        "#;
        let pkg = parse(src).unwrap().package.unwrap();
        assert_eq!(pkg.ns_uri, "http://test");
        assert_eq!(pkg.ns_prefix, "t");
        assert_eq!(pkg.annotation_directives.len(), 1);
        let node = pkg.class("Node").unwrap();
        assert_eq!(node.attributes.len(), 3);
        assert!(node.attributes[0].derived);
        assert!(node.attributes[0].getter_body.is_some());
        assert!(node.attributes[1].read_only);
        assert!(node.attributes[2].id);
    }

    #[test]
    fn parses_data_type_wraps() {
        let src = r#"
            package nodes
            type String wraps java.lang.String
            class Node { String[] property }
        "#;
        let pkg = parse(src).unwrap().package.unwrap();
        assert_eq!(pkg.data_types.len(), 1);
        assert_eq!(pkg.data_types[0].name, "String");
        assert_eq!(pkg.data_types[0].wrapped_class_name, "java.lang.String");
        assert!(pkg.class("Node").unwrap().attributes[0].multi);
    }

    #[test]
    fn default_ns_prefix_and_uri() {
        let pkg = parse("package com.example.demo").unwrap().package.unwrap();
        assert_eq!(pkg.ns_prefix, "demo");
        assert_eq!(pkg.ns_uri, "http://com.example.demo");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("package p { class }").is_err());
        assert!(parse("package p { class A {??} }").is_err());
    }
}
