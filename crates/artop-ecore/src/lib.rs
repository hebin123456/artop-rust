//! # artop-ecore
//!
//! Ecore metamodel types, ported from the C++ `emf-ecore` module of
//! `hebin123456/artop-cpp`. Provides the static metamodel objects that the
//! generated `artop-metamodel` registry and the dynamic EObject layer build on:
//! `EClassifier`, `EClass`, `EPackage`, `EFactory`.
//!
//! Inheritance is expressed the EMF way — through `EClass::e_super_types`
//! metadata — *not* through Rust subtyping, matching the porting strategy used
//! across this workspace (see `artop-metamodel` and `reflect`).

use artop_common::eobject::EObject;
use artop_common::uri::Uri;

/// A structural feature: an attribute or a reference (C++ `EStructuralFeature`).
#[derive(Debug, Clone, PartialEq)]
pub struct EStructuralFeature {
    /// Feature name, e.g. `"shortName"`.
    pub name: String,
    /// Whether it is a reference rather than an attribute.
    pub is_reference: bool,
    /// 1..* lower bound.
    pub lower_bound: i32,
    /// Upper bound; `-1` means unbounded.
    pub upper_bound: i32,
}

/// A classifier: an enum or a class (C++ `EClassifier`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EClassifier {
    kind: ClassifierKind,
}

impl EClassifier {
    /// New classifier of the given kind.
    pub fn new(kind: ClassifierKind) -> Self {
        Self { kind }
    }

    /// The classifier kind.
    pub fn kind(&self) -> ClassifierKind {
        self.kind
    }
}

/// Discriminator for a classifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassifierKind {
    /// A concrete class.
    Class,
    /// An abstract class (in the Rust port: no constructible struct).
    AbstractClass,
    /// An enum.
    Enum,
    /// A data type.
    DataType,
}

/// A class in the metamodel (C++ `EClass`).
#[derive(Debug, Clone, PartialEq)]
pub struct EClass {
    name: String,
    kind: ClassifierKind,
    // Multiple inheritance is a *list*, exactly like EMF `eSuperTypes`.
    supers: Vec<String>,
    own_features: Vec<EStructuralFeature>,
}

impl EClass {
    /// New class with metadata captured by the generator.
    pub fn new(
        name: impl Into<String>,
        kind: ClassifierKind,
        supers: Vec<String>,
        own_features: Vec<EStructuralFeature>,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            supers,
            own_features,
        }
    }

    /// Class name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `eSuperTypes`: parent class names (may be several — multiple inheritance).
    pub fn e_super_types(&self) -> &[String] {
        &self.supers
    }

    /// Own (locally declared) structural features.
    pub fn e_structural_features(&self) -> &[EStructuralFeature] {
        &self.own_features
    }

    /// Abstract classes produce no constructible Rust type.
    pub fn is_abstract(&self) -> bool {
        self.kind == ClassifierKind::AbstractClass
    }

    /// EMF `eAllFeatures()` over this class's metadata chain is provided by the
    /// `reflect` module in `artop-metamodel`.
    pub fn own_feature_names(&self) -> impl Iterator<Item = &str> {
        self.own_features.iter().map(|f| f.name.as_str())
    }
}

impl EObject for EClass {
    fn e_class(&self) -> &'static str {
        "EClass"
    }
}

/// A package in the metamodel (C++ `EPackage`).
#[derive(Debug, Clone)]
pub struct EPackage {
    name: String,
    ns_uri: Option<Uri>,
    classifiers: Vec<EClassifier>,
}

impl EPackage {
    /// New package.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ns_uri: None,
            classifiers: Vec::new(),
        }
    }

    /// Package name, e.g. `"autosar40"`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Namespace URI, e.g. `http://autosar.org/schema/r4.0/autosar40`.
    pub fn ns_uri(&self) -> Option<&Uri> {
        self.ns_uri.as_ref()
    }

    /// Set the namespace URI.
    pub fn set_ns_uri(&mut self, ns_uri: impl Into<String>) {
        self.ns_uri = Some(Uri::new(ns_uri));
    }

    /// Classifiers owned by this package.
    pub fn classifiers(&self) -> &[EClassifier] {
        &self.classifiers
    }
}

impl EObject for EPackage {
    fn e_class(&self) -> &'static str {
        "EPackage"
    }
}

/// Factory entry point for creating model objects from a classifier
/// (C++ `EFactory`, EMF `EFactory`).
#[derive(Debug, Clone, Copy)]
pub struct EFactory;

impl EFactory {
    /// Marker: creation strategies (`create` on classifier) are wired here
    /// once `artop-metamodel` provides the registry.
    pub const KIND: &'static str = "artop-ecore.EFactory";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eclass_metadata_roundtrip() {
        let c = EClass::new(
            "ARElement",
            ClassifierKind::Class,
            vec!["Identifiable".to_string()],
            vec![EStructuralFeature {
                name: "shortName".into(),
                is_reference: false,
                lower_bound: 0,
                upper_bound: 1,
            }],
        );
        assert_eq!(c.name(), "ARElement");
        assert!(!c.is_abstract());
        assert_eq!(c.e_super_types(), &["Identifiable"]);
        assert_eq!(c.own_feature_names().collect::<Vec<_>>(), ["shortName"]);
        assert_eq!(c.e_class(), "EClass");
    }

    #[test]
    fn abstract_class_has_no_constructible_type() {
        let a = EClass::new("ARObject", ClassifierKind::AbstractClass, vec![], vec![]);
        assert!(a.is_abstract());
    }
}
