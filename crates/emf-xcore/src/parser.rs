//! Recursive-descent parser for the Xcore DSL.
//!
//! The grammar supported here is the widely-used textual EMF notation. A
//! minimal example:
//!
//! ```xcore
//! @GenModel
//! package books {
//!   @UUID
//!   class Book extends NamedElement {
//!     String title
//!     Book #chapters     // `#` marks a containment reference
//!   }
//! }
//! ```
//!
//! Concrete grammar nodes (see [`dsl`] for the AST):
//!
//! - Annotations are `@key` or `@key.value` lines.
//! - A package is `package name { decl* }`.
//! - A class is `[interface|abstract class|class] Name [extends A, B] { feature* }`.
//! - A feature is `Type [multiplicity] [#] name [= default]`, where a leading
//!   `#` marks a containment reference. The multiplicity symbol (`?`, `*`, `+`)
//!   sits between the type and the name.
//! - `@DataType Name [= BackingType]` declares a data type.
//! - `@Enum Name { A [= 0], B, ... }` declares an enumeration.

use std::collections::BTreeMap;

use crate::dsl::{
    Annotation, DataTypeDecl, EClassDecl, EEnumDecl, EEnumLiteralDecl, FeatureDecl, FeatureKind,
    Multiplicity, PackageDecl, TypedElement,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFile {
    /// Annotations attached at file scope.
    pub annotations: Vec<Annotation>,
    /// The top-level package declaration, if present.
    pub package: Option<PackageDecl>,
}

impl ParsedFile {
    /// Convenience accessor for the package class list.
    pub fn classes(&self) -> &[EClassDecl] {
        self.package.as_ref().map_or(&[], |p| &p.classes)
    }
}

/// Built-in primitive type names that classify a feature as an attribute.
const BUILTIN_DATA_TYPES: &[&str] = &[
    "String",
    "Boolean",
    "Int",
    "Integer",
    "EInt",
    "Long",
    "ELong",
    "Short",
    "EShort",
    "Byte",
    "EByte",
    "Float",
    "EFloat",
    "Double",
    "EDouble",
    "BigDecimal",
    "BigInteger",
    "Date",
    "EString",
    "EDate",
    "EChar",
    "Char",
    "EBoolean",
    "EBigDecimal",
    "EBigInteger",
    "Object",
];

/// Classify a feature's kind. A `#` prefix forces containment reference;
/// otherwise we use the package data-type registry to disambiguate.
fn resolve_kind(force_reference: bool, type_name: &str, datatypes: &[String]) -> FeatureKind {
    if force_reference {
        FeatureKind::Reference
    } else if BUILTIN_DATA_TYPES.contains(&type_name) || datatypes.iter().any(|d| d == type_name) {
        FeatureKind::Attribute
    } else {
        FeatureKind::Reference
    }
}

/// Resolve each feature's kind now that the whole package is known.
fn resolve_kinds(pkg: &mut PackageDecl) {
    let datatypes: Vec<String> = pkg
        .data_types
        .iter()
        .flat_map(|d| {
            let mut names = vec![d.name.clone()];
            if let Some(ic) = &d.instance_class {
                names.push(ic.clone());
            }
            names
        })
        .collect();
    for class in &mut pkg.classes {
        for f in &mut class.features {
            f.kind = resolve_kind(f.containment, &f.ty.type_name, &datatypes);
        }
    }
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    KwPackage,
    KwClass,
    KwInterface,
    KwAbstract,
    KwExtends,
    At(String),
    Hash,
    LBrace,
    RBrace,
    Comma,
    Eq,
    Question,
    Star,
    Plus,
    Colon,
    StringLit(String),
    IntLit(i64),
    Eof,
}

impl Lexer {
    fn new(src: &str) -> Self {
        Self {
            chars: src.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                Some('/') if self.chars.get(self.pos + 1) == Some(&'/') => {
                    while let Some(c) = self.bump() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                Some('/') if self.chars.get(self.pos + 1) == Some(&'*') => {
                    self.bump();
                    self.bump();
                    while let Some(c) = self.bump() {
                        if c == '*' && self.peek() == Some('/') {
                            self.bump();
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

    fn tokenize(self) -> Result<Vec<(Tok, usize, usize)>, ParseError> {
        let mut lexer = self;
        let mut out = Vec::new();
        loop {
            lexer.skip_trivia();
            let (line, column) = (lexer.line, lexer.col);
            let c = match lexer.peek() {
                Some(c) => c,
                None => {
                    out.push((Tok::Eof, line, column));
                    break;
                }
            };
            let tok = match c {
                '{' => {
                    lexer.bump();
                    Tok::LBrace
                }
                '}' => {
                    lexer.bump();
                    Tok::RBrace
                }
                ',' => {
                    lexer.bump();
                    Tok::Comma
                }
                '=' => {
                    lexer.bump();
                    Tok::Eq
                }
                '#' => {
                    lexer.bump();
                    Tok::Hash
                }
                '?' => {
                    lexer.bump();
                    Tok::Question
                }
                '*' => {
                    lexer.bump();
                    Tok::Star
                }
                '+' => {
                    lexer.bump();
                    Tok::Plus
                }
                ':' => {
                    lexer.bump();
                    Tok::Colon
                }
                '@' => {
                    lexer.bump();
                    let mut key = String::new();
                    while let Some(ch) = lexer.peek() {
                        if is_ident_char(ch) {
                            key.push(ch);
                            lexer.bump();
                        } else {
                            break;
                        }
                    }
                    if key.is_empty() {
                        return Err(ParseError {
                            line,
                            column,
                            message: "expected annotation name after '@'".into(),
                        });
                    }
                    Tok::At(key)
                }
                '"' => {
                    lexer.bump();
                    let mut s = String::new();
                    loop {
                        match lexer.bump() {
                            Some('"') => break,
                            Some('\\') => match lexer.bump() {
                                Some('n') => s.push('\n'),
                                Some('t') => s.push('\t'),
                                Some('r') => s.push('\r'),
                                Some(other) => s.push(other),
                                None => {
                                    return Err(ParseError {
                                        line,
                                        column,
                                        message: "unterminated string literal".into(),
                                    })
                                }
                            },
                            Some(ch) => s.push(ch),
                            None => {
                                return Err(ParseError {
                                    line,
                                    column,
                                    message: "unterminated string literal".into(),
                                })
                            }
                        }
                    }
                    Tok::StringLit(s)
                }
                ch if ch.is_ascii_digit() || ch == '-' => {
                    let mut num = String::new();
                    while let Some(ch) = lexer.peek() {
                        if ch.is_ascii_digit() || (ch == '-' && num.is_empty()) {
                            num.push(ch);
                            lexer.bump();
                        } else {
                            break;
                        }
                    }
                    let parsed = num.parse::<i64>().map_err(|_| ParseError {
                        line,
                        column,
                        message: format!("invalid integer literal `{num}`"),
                    })?;
                    Tok::IntLit(parsed)
                }
                ch if is_ident_start(ch) => {
                    let mut name = String::new();
                    while let Some(ch) = lexer.peek() {
                        if is_ident_char(ch) {
                            name.push(ch);
                            lexer.bump();
                        } else {
                            break;
                        }
                    }
                    match name.as_str() {
                        "package" => Tok::KwPackage,
                        "class" => Tok::KwClass,
                        "interface" => Tok::KwInterface,
                        "abstract" => Tok::KwAbstract,
                        "extends" => Tok::KwExtends,
                        _ => Tok::Ident(name),
                    }
                }
                other => {
                    return Err(ParseError {
                        line,
                        column,
                        message: format!("unexpected character `{other}`"),
                    })
                }
            };
            out.push((tok, line, column));
        }
        Ok(out)
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == '.'
}

struct Parser {
    toks: Vec<(Tok, usize, usize)>,
    pos: usize,
}

type PResult<T> = Result<T, ParseError>;

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos].0
    }

    fn err(&self) -> ParseError {
        let (_, line, column) = self.toks[self.pos];
        ParseError {
            line,
            column,
            message: format!("unexpected token `{:?}`", self.peek()),
        }
    }

    fn advance(&mut self) {
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
    }

    fn expect(&mut self, tok: &Tok, _what: &str) -> PResult<()> {
        if self.peek() == tok {
            self.advance();
            Ok(())
        } else {
            Err(self.err())
        }
    }

    fn expect_ident(&mut self) -> PResult<String> {
        match self.peek().clone() {
            Tok::Ident(s) => {
                self.advance();
                Ok(s)
            }
            _ => Err(self.err()),
        }
    }

    fn eat_comma(&mut self) -> bool {
        if matches!(self.peek(), Tok::Comma) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Consume `@key` / `@key.value` annotations that directly precede a decl.
    /// Stops at the special `@DataType` / `@Enum` declaration markers.
    fn annotations(&mut self) -> PResult<Vec<Annotation>> {
        let mut out = Vec::new();
        while let Tok::At(key) = self.peek().clone() {
            let base = key.split('.').next().unwrap_or("");
            if base == "DataType" || base == "Enum" {
                break;
            }
            self.advance();
            let (name, value) = match key.split_once('.') {
                Some((n, v)) => (n.to_string(), Some(v.to_string())),
                None => (key.clone(), None),
            };
            out.push(Annotation {
                key: name,
                value,
                details: BTreeMap::new(),
            });
        }
        Ok(out)
    }

    /// Match an `@DataType` / `@Enum` declaration marker and return its base.
    fn at_decl(&mut self, base: &str) -> PResult<()> {
        match self.peek().clone() {
            Tok::At(key)
                if key == base
                    || key == format!("{base}.")
                    || key.starts_with(&format!("{base}.")) =>
            {
                self.advance();
                Ok(())
            }
            _ => Err(self.err()),
        }
    }

    fn package(&mut self, anns: Vec<Annotation>) -> PResult<PackageDecl> {
        self.expect(&Tok::KwPackage, "expected `package`")?;
        let name = self.expect_ident()?;
        let mut pkg = PackageDecl {
            name,
            annotations: anns,
            ..Default::default()
        };
        self.expect(&Tok::LBrace, "expected `{` to open package body")?;
        loop {
            let anns = self.annotations()?;
            match self.peek() {
                Tok::KwClass | Tok::KwInterface | Tok::KwAbstract => {
                    pkg.classes.push(self.class_(anns)?);
                }
                Tok::At(key) if key == "DataType" || key.starts_with("DataType.") => {
                    pkg.data_types.push(self.data_type(anns)?);
                }
                Tok::At(key) if key == "Enum" || key.starts_with("Enum.") => {
                    pkg.enums.push(self.enum_(anns)?);
                }
                Tok::RBrace => {
                    self.advance();
                    break;
                }
                Tok::Eof => break,
                _ => return Err(self.err()),
            }
        }
        Ok(pkg)
    }

    fn class_(&mut self, anns: Vec<Annotation>) -> PResult<EClassDecl> {
        let (mut interface, mut abstract_, mut concrete) = (false, false, false);
        match self.peek() {
            Tok::KwInterface => {
                interface = true;
                self.advance();
            }
            Tok::KwAbstract => {
                abstract_ = true;
                self.advance();
                self.expect(&Tok::KwClass, "expected `class` after `abstract`")?;
            }
            Tok::KwClass => {
                concrete = true;
                self.advance();
            }
            _ => return Err(self.err()),
        }
        let name = self.expect_ident()?;
        let mut super_types = Vec::new();
        if matches!(self.peek(), Tok::KwExtends) {
            self.advance();
            loop {
                super_types.push(self.expect_ident()?);
                if !self.eat_comma() {
                    break;
                }
            }
        }
        let mut features = Vec::new();
        if matches!(self.peek(), Tok::LBrace) {
            self.advance();
            loop {
                let fanns = self.annotations()?;
                match self.peek() {
                    Tok::RBrace => {
                        self.advance();
                        break;
                    }
                    Tok::Eof => break,
                    _ => features.push(self.feature(fanns)?),
                }
            }
        }
        Ok(EClassDecl {
            name,
            concrete,
            interface,
            abstract_,
            super_types,
            features,
            annotations: anns,
        })
    }

    fn data_type(&mut self, anns: Vec<Annotation>) -> PResult<DataTypeDecl> {
        self.at_decl("DataType")?;
        let name = self.expect_ident()?;
        let mut instance_class = None;
        if matches!(self.peek(), Tok::Eq) {
            self.advance();
            instance_class = Some(self.expect_ident()?);
        }
        Ok(DataTypeDecl {
            name,
            instance_class,
            annotations: anns,
            serializable: true,
        })
    }

    fn enum_(&mut self, anns: Vec<Annotation>) -> PResult<EEnumDecl> {
        self.at_decl("Enum")?;
        let name = self.expect_ident()?;
        let mut literals = Vec::new();
        if matches!(self.peek(), Tok::LBrace) {
            self.advance();
            loop {
                if matches!(self.peek(), Tok::RBrace) {
                    self.advance();
                    break;
                }
                let lit_name = self.expect_ident()?;
                let value = if matches!(self.peek(), Tok::Eq) {
                    self.advance();
                    match self.peek().clone() {
                        Tok::IntLit(v) => {
                            self.advance();
                            Some(v as i32)
                        }
                        Tok::Ident(i) => {
                            self.advance();
                            i.parse::<i32>().ok()
                        }
                        _ => return Err(self.err()),
                    }
                } else {
                    None
                };
                literals.push(EEnumLiteralDecl {
                    name: lit_name,
                    value,
                    literal: None,
                });
                if !self.eat_comma() && !matches!(self.peek(), Tok::RBrace) {
                    return Err(self.err());
                }
            }
        }
        Ok(EEnumDecl {
            name,
            annotations: anns,
            literals,
            ..Default::default()
        })
    }

    /// Parse one feature: `Type [mult] [#] name [= default]`.
    fn feature(&mut self, _anns: Vec<Annotation>) -> PResult<FeatureDecl> {
        let type_name = self.expect_ident()?;
        let multiplicity = self.multiplicity();
        let containment = if matches!(self.peek(), Tok::Hash) {
            self.advance();
            true
        } else {
            false
        };
        let name = self.expect_ident()?;
        let default = if matches!(self.peek(), Tok::Eq) {
            self.advance();
            match self.peek().clone() {
                Tok::StringLit(s) => {
                    self.advance();
                    Some(s)
                }
                Tok::Ident(s) => {
                    self.advance();
                    Some(s)
                }
                Tok::IntLit(i) => {
                    self.advance();
                    Some(i.to_string())
                }
                _ => return Err(self.err()),
            }
        } else {
            None
        };
        Ok(FeatureDecl {
            kind: FeatureKind::Attribute, // resolved by `resolve_kinds`
            name,
            ty: TypedElement {
                type_name,
                multiplicity,
            },
            containment,
            const_flag: false,
            unique: true,
            ordered: true,
            default,
            annotations: Vec::new(),
        })
    }

    fn multiplicity(&mut self) -> Multiplicity {
        match self.peek() {
            Tok::Question => {
                self.advance();
                Multiplicity::ZeroToOne
            }
            Tok::Star => {
                self.advance();
                Multiplicity::ZeroToMany
            }
            Tok::Plus => {
                self.advance();
                Multiplicity::OneToMany
            }
            _ => Multiplicity::One,
        }
    }
}

/// Parse Xcore source text into a [`ParsedFile`].
pub fn parse(src: &str) -> Result<ParsedFile, ParseError> {
    let lexer = Lexer::new(src);
    let toks = lexer.tokenize()?;
    let mut p = Parser { toks, pos: 0 };
    let mut file = ParsedFile {
        annotations: p.annotations()?,
        package: None,
    };
    if matches!(p.peek(), Tok::KwPackage) {
        let mut pkg = p.package(std::mem::take(&mut file.annotations))?;
        resolve_kinds(&mut pkg);
        file.package = Some(pkg);
    }
    if p.peek() != &Tok::Eof {
        return Err(p.err());
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_class() {
        let src = "package books {\n\
            class Book extends NamedElement {\n\
                String title\n\
                Book #chapters\n\
            }\n\
        }";
        let file = parse(src).expect("parse ok");
        let pkg = file.package.unwrap();
        assert_eq!(pkg.name, "books");
        let book = pkg.class("Book").unwrap();
        assert!(book.concrete);
        assert_eq!(book.super_types, vec!["NamedElement".to_string()]);
        assert_eq!(book.features.len(), 2);
        assert_eq!(book.features[0].kind, FeatureKind::Attribute);
        assert_eq!(book.features[0].name, "title");
        assert_eq!(book.features[0].ty.multiplicity, Multiplicity::One);
        assert_eq!(book.features[1].kind, FeatureKind::Reference);
        assert!(book.features[1].containment);
        assert_eq!(book.features[1].name, "chapters");
    }

    #[test]
    fn parses_multiplicity_and_datatypes() {
        let src = "package m {\n\
            @DataType String = java.lang.String\n\
            class C {\n\
                String *names\n\
                Int? count\n\
            }\n\
            @Enum Color { RED, GREEN = 1, BLUE }\n\
        }";
        let file = parse(src).expect("parse ok");
        let pkg = file.package.unwrap();
        assert_eq!(pkg.data_types.len(), 1);
        assert_eq!(
            pkg.data_types[0].instance_class.as_deref(),
            Some("java.lang.String")
        );
        let c = pkg.class("C").unwrap();
        assert_eq!(c.features[0].ty.multiplicity, Multiplicity::ZeroToMany);
        assert!(c.features[0].ty.multiplicity.is_many());
        assert_eq!(c.features[1].ty.multiplicity, Multiplicity::ZeroToOne);
        assert!(c.features[1].ty.multiplicity.is_optional());
        let enu = pkg.enums.first().unwrap();
        assert_eq!(enu.literals.len(), 3);
        assert_eq!(enu.literals[1].value, Some(1));
    }

    #[test]
    fn parses_annotations() {
        let src = "@GenModel\npackage p {\n    @UUID\n    class A { }\n}";
        let file = parse(src).expect("parse ok");
        let pkg = file.package.unwrap();
        assert_eq!(pkg.annotations[0].key, "GenModel");
        assert_eq!(pkg.class("A").unwrap().annotations[0].key, "UUID");
    }

    #[test]
    fn parses_abstract_interface() {
        let src = "package p {\n\
            interface IFace\n\
            abstract class Base\n\
            class Impl extends Base, IFace\n\
        }";
        let file = parse(src).expect("parse ok");
        let pkg = file.package.unwrap();
        assert!(pkg.class("IFace").unwrap().interface);
        assert!(pkg.class("Base").unwrap().abstract_);
        assert!(pkg.class("Impl").unwrap().is_instantiable());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("package p { class }").is_err());
        assert!(parse("package p { class A {??} }").is_err());
    }
}
