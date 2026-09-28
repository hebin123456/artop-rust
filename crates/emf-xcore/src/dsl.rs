//! Abstract syntax tree for the Xcore DSL.
//!
//! Port of C++ `emf-ecore/xcore/XcoreAst.h` (aligned to Java
//! `org.eclipse.emf.ecore.xcore`). The types model the elements of an `.xcore`
//! file: a top-level package containing classes, data types and enums, each
//! with their members (attributes, references, operations) and annotations.
//!
//! The AST holds *source-level* information only; the Ecore metamodel instances
//! are derived from it by [`crate::generator::XcoreGenerator`].

/// Syntactic keyword prefix of a reference declaration (C++ `ReferenceKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReferenceKind {
    /// `contains` — a containment reference.
    Containment,
    /// `refers` — a non-containment reference.
    NonContainment,
    /// No keyword (equivalent to `contains` when the target is an `EClass`).
    #[default]
    Plain,
}

/// `annotation "uri" as Name` — a named annotation directive (C++
/// `XAnnotationDirective`, Java `XAnnotationDirective`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnnotationDirective {
    /// The alias after `as`.
    pub name: String,
    /// The URI string after `annotation`, used as the derived `EAnnotation.source`.
    pub source_uri: String,
}

/// `@Directive` / `@Directive(key=value, ...)` — an annotation (C++
/// `XAnnotation`, Java `XAnnotation`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Annotation {
    /// The referenced directive alias (the name after `@`).
    pub directive_name: String,
    /// Ordered `key=value` details.
    pub details: Vec<(String, String)>,
}

impl Annotation {
    /// New annotation with only a directive name.
    pub fn new(directive_name: impl Into<String>) -> Self {
        Self {
            directive_name: directive_name.into(),
            details: Vec::new(),
        }
    }

    /// Look up a detail value by key.
    pub fn detail(&self, key: &str) -> Option<&str> {
        self.details
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Set/replace a detail entry.
    pub fn set_detail(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        if let Some(slot) = self.details.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            self.details.push((key, value));
        }
    }
}

/// An attribute declaration (C++ `XAttribute`, Java `XAttribute`).
///
/// Syntax: `[final|readonly|volatile|transient|unsettable|derived|id] Type[multi]? name [= default] [get { body }]`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AttributeDecl {
    /// Attribute name.
    pub name: String,
    /// Xcore type name (`String`, `int`, an enum name, ...).
    pub type_name: String,
    /// Whether the `[]` (or equivalent) multiplicity suffix was used.
    pub multi: bool,
    /// Whether `derived`.
    pub derived: bool,
    /// Whether `transient`.
    pub transient: bool,
    /// Whether `unsettable`.
    pub unsettable: bool,
    /// Whether `readonly`.
    pub read_only: bool,
    /// Whether `volatile`.
    pub volatile: bool,
    /// Whether `id`.
    pub id: bool,
    /// Optional default value literal.
    pub default_value_literal: Option<String>,
    /// Optional derived getter body (`derived long x get { ... }`).
    pub getter_body: Option<String>,
    /// Annotations attached to the attribute.
    pub annotations: Vec<Annotation>,
}

/// A reference declaration (C++ `XReference`, Java `XReference`).
///
/// Syntax: `[contains|refers|readonly|volatile|transient|unsettable|derived|resolve] Type[multi]? name [opposite Name] [get { body }]`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReferenceDecl {
    /// Reference name.
    pub name: String,
    /// Target type name.
    pub type_name: String,
    /// Syntactic keyword prefix.
    pub kind: ReferenceKind,
    /// Whether the `[]` (or equivalent) multiplicity suffix was used.
    pub multi: bool,
    /// Whether `derived`.
    pub derived: bool,
    /// Whether `transient`.
    pub transient: bool,
    /// Whether `unsettable`.
    pub unsettable: bool,
    /// Whether `readonly`.
    pub read_only: bool,
    /// Whether `volatile`.
    pub volatile: bool,
    /// Whether `resolveProxies` (default `true`).
    pub resolve_proxies: bool,
    /// Optional `opposite Name`.
    pub opposite_name: Option<String>,
    /// Optional derived getter body.
    pub getter_body: Option<String>,
    /// Annotations attached to the reference.
    pub annotations: Vec<Annotation>,
}

/// A parameter of an operation (C++ `XParameter`, Java `XParameter`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParameterDecl {
    /// Parameter name.
    pub name: String,
    /// Parameter type name.
    pub type_name: String,
}

/// An operation declaration (C++ `XOperation`, Java `XOperation`).
///
/// Syntax: `op ReturnType name(params) [throws E1, E2] { body }`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OperationDecl {
    /// Operation name.
    pub name: String,
    /// Return type name.
    pub type_name: String,
    /// Declared parameters, in order.
    pub parameters: Vec<ParameterDecl>,
    /// Declared `throws` exception type names.
    pub exceptions: Vec<String>,
    /// Optional operation body text (kept verbatim, not compiled).
    pub body: Option<String>,
    /// Annotations attached to the operation.
    pub annotations: Vec<Annotation>,
}

/// An enumeration literal declaration (C++ `XEnumLiteral`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnumLiteralDecl {
    /// Literal name.
    pub name: String,
    /// Explicit or auto-incremented integer value.
    pub value: Option<i32>,
    /// Literal string (defaults to the name).
    pub literal: String,
    /// Annotations attached to the literal.
    pub annotations: Vec<Annotation>,
}

/// An enumeration declaration (C++ `XEnum`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnumDecl {
    /// Enumeration name.
    pub name: String,
    /// Literals, in declaration order.
    pub literals: Vec<EnumLiteralDecl>,
    /// Annotations.
    pub annotations: Vec<Annotation>,
}

/// A data type declaration (C++ `XDataType`): `type Name wraps java.lang.Type`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataTypeDecl {
    /// Data type name.
    pub name: String,
    /// The wrapped Java class name after `wraps`.
    pub wrapped_class_name: String,
    /// Annotations.
    pub annotations: Vec<Annotation>,
}

/// A class declaration (C++ `XClass`, Java `XClass`).
///
/// Syntax: `[abstract|interface] class Name [extends Super1, Super2] { members }`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClassDecl {
    /// Class name.
    pub name: String,
    /// Whether `abstract` was used.
    pub is_abstract: bool,
    /// Whether `interface` was used.
    pub is_interface: bool,
    /// Super-type names from `extends`.
    pub super_types: Vec<String>,
    /// Attribute members.
    pub attributes: Vec<AttributeDecl>,
    /// Reference members.
    pub references: Vec<ReferenceDecl>,
    /// Operation members.
    pub operations: Vec<OperationDecl>,
    /// Annotations.
    pub annotations: Vec<Annotation>,
}

impl ClassDecl {
    /// Whether instances of this class may be created.
    pub fn is_instantiable(&self) -> bool {
        !self.is_abstract && !self.is_interface
    }
}

/// A top-level package declaration (C++ `XPackage`, Java `XPackage`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageDecl {
    /// Package qualified name, e.g. `books` or `com.example.books`.
    pub name: String,
    /// The namespace URI (`@Ecore(nsURI=...)`), defaulted by the parser.
    pub ns_uri: String,
    /// The namespace prefix (`@Ecore(nsPrefix=...)`), defaulted by the parser.
    pub ns_prefix: String,
    /// Annotation directives (`annotation "uri" as Name`).
    pub annotation_directives: Vec<AnnotationDirective>,
    /// Class declarations.
    pub classes: Vec<ClassDecl>,
    /// Enum declarations.
    pub enums: Vec<EnumDecl>,
    /// Data type declarations.
    pub data_types: Vec<DataTypeDecl>,
    /// Annotations attached at package scope.
    pub annotations: Vec<Annotation>,
}

impl PackageDecl {
    /// Find a class declaration by name.
    pub fn class(&self, name: &str) -> Option<&ClassDecl> {
        self.classes.iter().find(|c| c.name == name)
    }

    /// Resolve an annotation directive's `source_uri` by alias.
    pub fn directive_source(&self, name: &str) -> Option<&str> {
        self.annotation_directives
            .iter()
            .find(|d| d.name == name)
            .map(|d| d.source_uri.as_str())
    }
}
