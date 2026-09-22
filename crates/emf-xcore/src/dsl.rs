//! Abstract syntax tree for the Xcore DSL.
//!
//! The types in this module model the elements of an `.xcore` file: a top-level
//! package containing classes (`@Entity`), data types (`@DataType`), enums
//! (`@Enum`) and their members. Each declaration may carry a set of annotations
//! (`@X0.1`, `@GenModel`, ...) and attribute/member flags such as
//! `class`, `interface`, `abstract`, `const`, `unique`, `ordered`.

use std::collections::BTreeMap;

/// A single `@key.value` annotation attached to a declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    /// Annotation identifier/namespace, e.g. `GenModel`.
    pub key: String,
    /// Optional value after the dot, e.g. `@GenModel.roots` -> value `roots`.
    pub value: Option<String>,
    /// Free-form attribute map for extended annotations.
    pub details: BTreeMap<String, String>,
}

impl Annotation {
    /// Short-hand constructor for a key-only annotation.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: None,
            details: BTreeMap::new(),
        }
    }
}

/// Multiplicity / cardinality of a structural feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Multiplicity {
    /// Exactly one, not optional (default).
    #[default]
    One,
    /// Zero or one (`?`).
    ZeroToOne,
    /// One or many (`+`).
    OneToMany,
    /// Zero or many (`*`).
    ZeroToMany,
}

impl Multiplicity {
    /// Whether the feature can hold more than one value.
    pub fn is_many(self) -> bool {
        matches!(self, Multiplicity::OneToMany | Multiplicity::ZeroToMany)
    }

    /// Whether the feature may be absent.
    pub fn is_optional(self) -> bool {
        matches!(self, Multiplicity::ZeroToOne | Multiplicity::ZeroToMany)
    }
}

/// A typed element: either a reference or an attribute. Both carry a `type`
/// name (resolved through the package on demand).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedElement {
    /// The declared type of the element, e.g. `String` or `::model::Book`.
    pub type_name: String,
    /// Cardinality of the element.
    pub multiplicity: Multiplicity,
}

/// A structural feature declaration inside an entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureDecl {
    /// Feature kind.
    pub kind: FeatureKind,
    /// Name of the feature (camelCase).
    pub name: String,
    /// The referenced type.
    pub ty: TypedElement,
    /// Whether `#` containment modifier was used (containment reference).
    pub containment: bool,
    /// Whether `const` modifier was used.
    pub const_flag: bool,
    /// Modifier `unique` / `ordered` toggles.
    pub unique: bool,
    /// Modifier `ordered` toggles.
    pub ordered: bool,
    /// Optional default literal.
    pub default: Option<String>,
    /// Annotations attached to the feature.
    pub annotations: Vec<Annotation>,
}

/// Distinguishes attribute features from reference features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureKind {
    /// Data-valued attribute.
    Attribute,
    /// Reference to another EObject.
    Reference,
}

impl FeatureDecl {
    /// Whether this is a reference feature.
    pub fn is_reference(&self) -> bool {
        self.kind == FeatureKind::Reference
    }
}

/// An enumeration literal declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EEnumLiteralDecl {
    /// Literal name.
    pub name: String,
    /// Optional integer value.
    pub value: Option<i32>,
    /// Optional literal string.
    pub literal: Option<String>,
}

/// An enumeration declaration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EEnumDecl {
    /// Enumeration name.
    pub name: String,
    /// Whether `interface` (uninstantiable) was used.
    pub interface: bool,
    /// Whether `abstract` was used.
    pub abstract_: bool,
    /// Annotations.
    pub annotations: Vec<Annotation>,
    /// Literals, in declaration order.
    pub literals: Vec<EEnumLiteralDecl>,
}

/// A data type declaration (`@DataType`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataTypeDecl {
    /// Data type name.
    pub name: String,
    /// Optional Java/Rust backing type, e.g. `String`.
    pub instance_class: Option<String>,
    /// Annotations.
    pub annotations: Vec<Annotation>,
    /// Whether serializable.
    pub serializable: bool,
}

/// A class (entity) declaration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EClassDecl {
    /// Class name.
    pub name: String,
    /// Whether `class` (instantiable concrete) was used.
    pub concrete: bool,
    /// Whether `interface` was used.
    pub interface: bool,
    /// Whether `abstract` was used.
    pub abstract_: bool,
    /// Super-type names, e.g. `Book` or `::ecore::EObject`.
    pub super_types: Vec<String>,
    /// Structural features declared in the body.
    pub features: Vec<FeatureDecl>,
    /// Annotations.
    pub annotations: Vec<Annotation>,
}

impl EClassDecl {
    /// Whether instances of this class may be created.
    pub fn is_instantiable(&self) -> bool {
        self.concrete && !self.abstract_ && !self.interface
    }
}

/// A top-level package declaration (the root of a file).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageDecl {
    /// Package short name, e.g. `books`.
    pub name: String,
    /// The package ns URI, e.g. `http://example.com/books`.
    pub ns_uri: Option<String>,
    /// The package ns prefix, e.g. `books`.
    pub ns_prefix: Option<String>,
    /// The Java / Rust package id used to compute the code-gen package.
    pub package_id: Option<String>,
    /// Class declarations.
    pub classes: Vec<EClassDecl>,
    /// Data type declarations.
    pub data_types: Vec<DataTypeDecl>,
    /// Enumeration declarations.
    pub enums: Vec<EEnumDecl>,
    /// Annotations on the package itself.
    pub annotations: Vec<Annotation>,
}

impl PackageDecl {
    /// Find a class declaration by name.
    pub fn class(&self, name: &str) -> Option<&EClassDecl> {
        self.classes.iter().find(|c| c.name == name)
    }
}
