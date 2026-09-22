//! Classifiers: `EClassifier`, `EClass`, `EDataType`, `EEnum`, `EEnumLiteral`.
//!
//! Port of C++ `emf-ecore` interfaces (`EClassifier`/`EClass`/`EDataType`/
//! `EEnum`/`EEnumLiteral`). Inheritance is *metadata*: `EClass`'s `e_super_types`
//! lists parent names and the `e_all_*` query family walks the graph.

use crate::structural::{EOperation, EStructuralFeature};
use crate::Val;
use std::collections::HashSet;

/// Discriminator for how a `EClass` is instantiated / used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EClassKind {
    /// A concrete, instantiable class.
    Class,
    /// An abstract class (no direct instances).
    AbstractClass,
    /// An interface (no direct instances).
    Interface,
    /// A map entry (EMF `Map.Entry`).
    MapEntry,
}

impl EClassKind {
    /// Whether this kind permits direct instances.
    pub fn is_concrete(&self) -> bool {
        matches!(self, EClassKind::Class | EClassKind::MapEntry)
    }
}

impl Default for EClassKind {
    fn default() -> Self {
        EClassKind::Class
    }
}

/// A classifier: the abstract super-concept of `EClass` and `EDataType`.
///
/// In the Rust port this carries the common name + package linkage; the concrete
/// classifiers [`EClass`] and [`EDataType`] both expose this surface.
#[allow(dead_code)] // public API surface; lifecycle managed by concrete classifiers.
#[derive(Debug, Clone, Default)]
pub struct EClassifier {
    name: String,
    package: Option<crate::package::PackageRef>,
}

/// An enum literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EEnumLiteral {
    /// Literal name (EMF `EEnumLiteral.name`).
    name: String,
    /// The literal string (EMF `EEnumLiteral.literal`).
    literal: String,
    /// Ordinal value (EMF `EEnumLiteral.value`).
    value: i32,
}

impl EEnumLiteral {
    /// New literal.
    pub fn new(name: impl Into<String>, value: i32) -> Self {
        Self {
            name: name.into(),
            literal: String::new(),
            value,
        }
    }

    /// Literal name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Set the literal name.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }
    /// The literal-string form (`EEnumLiteral.literal`).
    pub fn literal(&self) -> &str {
        &self.literal
    }
    /// Set the literal string.
    pub fn set_literal(&mut self, literal: impl Into<String>) {
        self.literal = literal.into();
    }
    /// Ordinal value.
    pub fn value(&self) -> i32 {
        self.value
    }
    /// Set the ordinal value.
    pub fn set_value(&mut self, value: i32) {
        self.value = value;
    }
}

/// An enum data type with ordered literals.
#[derive(Debug, Clone, Default)]
pub struct EEnum {
    name: String,
    literals: Vec<EEnumLiteral>,
    serializable: bool,
}

impl EEnum {
    /// New empty enum.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            literals: Vec::new(),
            serializable: true,
        }
    }

    /// Enum name (EMF `ENamedElement.name`).
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Set the name.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }
    /// `eLiterals`.
    pub fn e_literals(&self) -> &[EEnumLiteral] {
        &self.literals
    }
    /// Mutable `eLiterals`.
    pub fn e_literals_mut(&mut self) -> &mut Vec<EEnumLiteral> {
        &mut self.literals
    }
    /// Append a literal.
    pub fn add_literal(&mut self, name: impl Into<String>, value: i32) -> &mut EEnumLiteral {
        self.literals.push(EEnumLiteral::new(name, value));
        self.literals.last_mut().unwrap()
    }
    /// Look up a literal by literal *name* (EMF `getELiteral(String)`).
    pub fn literal_by_name(&self, name: &str) -> Option<&EEnumLiteral> {
        self.literals.iter().find(|l| l.name() == name)
    }
    /// Look up a literal by ordinal value (EMF `getELiteral(int)`).
    pub fn literal_by_value(&self, value: i32) -> Option<&EEnumLiteral> {
        self.literals.iter().find(|l| l.value() == value)
    }
    /// Whether serializable.
    pub fn is_serializable(&self) -> bool {
        self.serializable
    }
    /// Set serializable.
    pub fn set_serializable(&mut self, serializable: bool) {
        self.serializable = serializable;
    }
}

/// A data type: a non-class classifier with a Java/instance class name.
#[derive(Debug, Clone, Default)]
pub struct EDataType {
    name: String,
    instance_class_name: String,
    serializable: bool,
}

impl EDataType {
    /// New data type.
    pub fn new(name: impl Into<String>, instance_class_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            instance_class_name: instance_class_name.into(),
            serializable: true,
        }
    }

    /// Data type name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Set the name.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }
    /// `instanceClassName`.
    pub fn instance_class_name(&self) -> &str {
        &self.instance_class_name
    }
    /// Set `instanceClassName`.
    pub fn set_instance_class_name(&mut self, name: impl Into<String>) {
        self.instance_class_name = name.into();
    }
    /// Whether serializable.
    pub fn is_serializable(&self) -> bool {
        self.serializable
    }
    /// Set serializable.
    pub fn set_serializable(&mut self, serializable: bool) {
        self.serializable = serializable;
    }
}

/// A class in the metamodel (C++ `EClass`).
#[derive(Debug, Clone, Default)]
pub struct EClass {
    name: String,
    kind: EClassKind,
    /// Parent class names (EMF `eSuperTypes`, may be several).
    super_types: Vec<String>,
    /// Locally declared structural features (own features).
    features: Vec<EStructuralFeature>,
    /// Locally declared operations.
    operations: Vec<EOperation>,
    /// Compiled fallback: default feature values keyed by feature id.
    default_values: Vec<(i32, Val)>,
    /// The FeatureID of the ID attribute, if the class has one.
    id_feature: Option<i32>,
}

impl EClass {
    /// New class.
    pub fn new(name: impl Into<String>, kind: EClassKind) -> Self {
        Self {
            name: name.into(),
            kind,
            ..Self::default()
        }
    }

    /// Class name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Set the class name.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }
    /// The class kind.
    pub fn kind(&self) -> EClassKind {
        self.kind
    }
    /// Abstract interface-agnostic: EClass.isAbstract (interfaces are abstract).
    pub fn is_abstract(&self) -> bool {
        !self.kind.is_concrete()
    }
    /// EClass.isInterface.
    pub fn is_interface(&self) -> bool {
        self.kind == EClassKind::Interface
    }
    /// EClass.isMapEntry.
    pub fn is_map_entry(&self) -> bool {
        self.kind == EClassKind::MapEntry
    }
    /// Enable the abstract flag.
    pub fn set_abstract(&mut self, abstract_: bool) {
        if self.kind.is_concrete() && abstract_ {
            self.kind = EClassKind::AbstractClass;
        } else if !self.kind.is_concrete() && !abstract_ {
            self.kind = EClassKind::Class;
        }
    }

    /// `eSuperTypes`: parent class names.
    pub fn e_super_types(&self) -> &[String] {
        &self.super_types
    }
    /// Append a super-type *name*; returns a cycle-guard error if it would
    /// create a direct self-loop.
    pub fn add_super_type(&mut self, name: impl Into<String>) -> Result<(), String> {
        let name = name.into();
        if name == self.name {
            return Err(format!("{} cannot inherit from itself", self.name));
        }
        if !self.super_types.contains(&name) {
            self.super_types.push(name);
        }
        Ok(())
    }
    /// Set the full super-type list.
    pub fn set_super_types(&mut self, supers: Vec<String>) {
        let mut supers = supers;
        let mut seen: HashSet<String> = HashSet::new();
        supers.retain(|s| {
            s != &self.name && !seen.contains(s) && {
                seen.insert(s.clone());
                true
            }
        });
        self.super_types = supers;
    }

    /// `eStructuralFeatures`: own features.
    pub fn e_structural_features(&self) -> &[EStructuralFeature] {
        &self.features
    }
    /// Mutable own features.
    pub fn e_structural_features_mut(&mut self) -> &mut Vec<EStructuralFeature> {
        &mut self.features
    }
    /// Append an own feature.
    pub fn add_feature(&mut self, feature: EStructuralFeature) {
        self.features.push(feature);
    }

    /// `eOperations`: own operations.
    pub fn e_operations(&self) -> &[EOperation] {
        &self.operations
    }
    /// Append an own operation.
    pub fn add_operation(&mut self, op: EOperation) {
        self.operations.push(op);
    }

    /// Register a default feature value keyed by feature id.
    pub fn set_default_value(&mut self, feature_id: i32, value: Val) {
        self.default_values.retain(|(f, _)| *f != feature_id);
        self.default_values.push((feature_id, value));
    }
    /// Default feature value by feature id.
    pub fn default_value(&self, feature_id: i32) -> Option<&Val> {
        self.default_values
            .iter()
            .rev()
            .find(|(f, _)| *f == feature_id)
            .map(|(_, v)| v)
    }

    /// Set which feature id is the ID attribute.
    pub fn set_id_feature(&mut self, feature_id: i32) {
        self.id_feature = Some(feature_id);
    }
    /// The ID attribute feature id, if any.
    pub fn id_feature(&self) -> Option<i32> {
        self.id_feature
    }

    // ---- reflection query family ----

    /// A flat ordered snapshot of this class's transitive ancestor names
    /// (deduplicated, ancestors-first, excludes `self`).
    pub fn e_all_super_types(&self, package: &crate::package::PackageRegistry) -> Vec<String> {
        self.all_super_type_names(package)
    }

    /// Is `sup` (by name) a *proper* ancestor of this class? A class is never
    /// considered its own super-type (EMF `isSuperTypeOf` is strict).
    pub fn is_super_type_of(&self, sup: &str, package: &crate::package::PackageRegistry) -> bool {
        self.all_super_type_names(package).iter().any(|s| s == sup)
    }

    /// `eAllStructuralFeatures`: inherited features (ancestors-first) followed
    /// by own features, deduplicated by feature id.
    pub fn e_all_structural_features(
        &self,
        package: &crate::package::PackageRegistry,
    ) -> Vec<EStructuralFeature> {
        let mut out: Vec<EStructuralFeature> = Vec::new();
        let mut seen: HashSet<i32> = HashSet::new();
        let mut push = |feats: &[EStructuralFeature]| {
            for f in feats {
                let fid = f.feature_id();
                if fid >= 0 && !seen.insert(fid) {
                    continue;
                }
                out.push(f.clone());
            }
        };
        for name in self.e_all_super_types(package) {
            if let Some(cls) = package.find_class(&name) {
                push(cls.e_structural_features());
            }
        }
        push(&self.features);
        out
    }

    /// `eAllAttributes`: the attributes among `eAllStructuralFeatures`.
    pub fn e_all_attributes(
        &self,
        package: &crate::package::PackageRegistry,
    ) -> Vec<EStructuralFeature> {
        self.e_all_structural_features(package)
            .into_iter()
            .filter(|f| !f.is_reference())
            .collect()
    }

    /// `eAllReferences`.
    pub fn e_all_references(
        &self,
        package: &crate::package::PackageRegistry,
    ) -> Vec<EStructuralFeature> {
        self.e_all_structural_features(package)
            .into_iter()
            .filter(|f| f.is_reference())
            .collect()
    }

    /// Find a structural feature by name across the whole hierarchy
    /// (EMF `getEStructuralFeature(String)`).
    pub fn feature_by_name(
        &self,
        name: &str,
        package: &crate::package::PackageRegistry,
    ) -> Option<EStructuralFeature> {
        self.e_all_structural_features(package)
            .into_iter()
            .find(|f| f.name() == name)
    }

    /// The FeatureID for a feature, searched across the whole hierarchy.
    /// Returns the feature's `feature_id`.
    pub fn feature_id_of(
        &self,
        name: &str,
        package: &crate::package::PackageRegistry,
    ) -> Option<i32> {
        self.feature_by_name(name, package).map(|f| f.feature_id())
    }

    /// A transitive-ancestor-name list (private helper).
    fn all_super_type_names(&self, package: &crate::package::PackageRegistry) -> Vec<String> {
        fn visit(cls: &EClass, out: &mut Vec<String>, pkg: &crate::package::PackageRegistry) {
            for sup in cls.e_super_types() {
                if out.iter().any(|s| s == sup) {
                    continue;
                }
                // push the parent before its own ancestors (ancestors-first)
                if let Some(parent) = pkg.find_class(sup) {
                    visit(&parent, out, pkg);
                }
                if !out.iter().any(|s| s == sup) {
                    out.push(sup.clone());
                }
            }
        }
        let mut out = Vec::new();
        visit(self, &mut out, package);
        out
    }
}
