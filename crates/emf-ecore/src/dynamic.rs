//! `DynamicEObject` — a reflective object backed by an `EClass` descriptor.
//!
//! Port of C++ `emf-ecore/DynamicEObject` (aligned to Java
//! `org.eclipse.emf.ecore.impl.DynamicEObjectImpl`). Values are stored in a
//! `HashMap<FeatureID, Val>` plus a set of "set" flags so `eIsSet` and
//! `eUnset` behave like EMF: unsetting restores the class's default value.

use crate::{EClass, Val};
use emf_common::eobject::EObject;
use std::collections::HashMap;

/// A reflective object whose storage is keyed by `FeatureID`.
///
/// `e_class()` returns the descriptor name (`EClass::name`). `e_get`/`e_set`/
/// `e_is_set`/`e_unset` operate on features by *name*, looking the feature up
/// across the class's `eAllStructuralFeatures`; values are stored in
/// `dynamic_settings`.
/// The package registry the object belongs to, used to resolve inherited
/// features. Bare (`None`) falls back to the process-wide global registry.
#[derive(Debug)]
pub struct DynamicEObject {
    /// The class descriptor (owned clone).
    pub e_class: EClass,
    /// Multi-purpose storage keyed by feature id.
    pub dynamic_settings: HashMap<i32, Val>,
    /// Whether each feature id has been set (for `eIsSet`).
    pub set_flags: std::collections::HashSet<i32>,
    /// Registry snapshot used to resolve the inheritance graph.
    registry: Option<crate::package::PackageRegistry>,
}

impl DynamicEObject {
    /// New object of the given class, resolving inheritance against the
    /// process-wide global registry.
    pub fn new(class: EClass) -> Self {
        Self {
            e_class: class,
            dynamic_settings: HashMap::new(),
            set_flags: std::collections::HashSet::new(),
            registry: None,
        }
    }

    /// New object of the given class, binding it to a specific registry so the
    /// class's inheritance graph resolves against that registry.
    pub fn new_in(class: EClass, registry: crate::package::PackageRegistry) -> Self {
        Self {
            e_class: class,
            dynamic_settings: HashMap::new(),
            set_flags: std::collections::HashSet::new(),
            registry: Some(registry),
        }
    }

    /// Bind (or rebind) the registry used for inheritance resolution.
    pub fn bind_registry(&mut self, registry: crate::package::PackageRegistry) {
        self.registry = Some(registry);
    }

    /// The class descriptor.
    pub fn class(&self) -> &EClass {
        &self.e_class
    }

    /// Read a feature by name. Returns `None` only when the feature name is not
    /// part of `eAllStructuralFeatures`; otherwise returns the stored value or,
    /// failing that, the class default.
    pub fn e_get_by_name(&self, name: &str) -> Option<Val> {
        let feature = self.e_all().into_iter().find(|f| f.name() == name)?;
        Some(self.e_get_feature(&feature))
    }

    /// Read a feature's value (stored or default).
    pub fn e_get_feature(&self, feature: &crate::structural::EStructuralFeature) -> Val {
        let id = feature.feature_id();
        if id >= 0 {
            if let Some(v) = self.dynamic_settings.get(&id) {
                return v.clone();
            }
            if let Some(d) = self.e_class.default_value(id) {
                return d.clone();
            }
        }
        feature.default_value().cloned().unwrap_or(Val::Null)
    }

    /// Set a feature by name. Returns false if the feature name is unknown.
    pub fn e_set_by_name(&mut self, name: &str, value: Val) -> bool {
        let feature = match self.e_all().into_iter().find(|f| f.name() == name) {
            Some(f) => f,
            None => return false,
        };
        let id = feature.feature_id();
        if id >= 0 {
            self.dynamic_settings.insert(id, value);
            self.set_flags.insert(id);
        }
        true
    }

    /// Whether a feature (by name) is set.
    pub fn e_is_set_by_name(&self, name: &str) -> Option<bool> {
        let feature = self.e_all().into_iter().find(|f| f.name() == name)?;
        Some(self.set_flags.contains(&(feature.feature_id())))
    }

    /// Unset a feature by name, restoring the class default. Returns false if
    /// the name is unknown or the feature is unsettable-only.
    pub fn e_unset_by_name(&mut self, name: &str) -> bool {
        let feature = match self.e_all().into_iter().find(|f| f.name() == name) {
            Some(f) => f,
            None => return false,
        };
        let id = feature.feature_id();
        if id >= 0 {
            self.dynamic_settings.remove(&id);
            self.set_flags.remove(&id);
        }
        true
    }

    /// All structural features (own + inherited), using the bound registry or
    /// the global registry as fallback.
    pub fn all_structural_features(&self) -> Vec<crate::structural::EStructuralFeature> {
        self.e_all()
    }

    /// All reference features (own + inherited), for cross-reference / serialization.
    pub fn all_references(&self) -> Vec<crate::structural::EStructuralFeature> {
        self.e_all()
            .into_iter()
            .filter(|f| f.is_reference())
            .collect()
    }

    /// All containment reference features (own + inherited), for serialization.
    pub fn all_containments(&self) -> Vec<crate::structural::EStructuralFeature> {
        self.e_all()
            .into_iter()
            .filter(|f| f.is_containment())
            .collect()
    }

    /// The bound registry used to resolve inheritance, if any.
    pub fn registry(&self) -> Option<&crate::package::PackageRegistry> {
        self.registry.as_ref()
    }

    /// All structural features (own + inherited), using the bound registry or
    /// the global registry as fallback.
    fn e_all(&self) -> Vec<crate::structural::EStructuralFeature> {
        let registry = self
            .registry
            .clone()
            .unwrap_or_else(crate::ecore_package::global);
        self.e_class.e_all_structural_features(&registry)
    }
}

impl EObject for DynamicEObject {
    fn e_class(&self) -> &str {
        self.e_class.name()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn e_get(&self, feature_id: &str) -> Option<Val> {
        self.e_get_by_name(feature_id)
    }

    fn e_set(&mut self, feature_id: &str, value: Val) -> bool {
        self.e_set_by_name(feature_id, value)
    }

    fn e_is_set(&self, feature_id: &str) -> bool {
        self.e_is_set_by_name(feature_id).unwrap_or(false)
    }

    fn e_unset(&mut self, feature_id: &str) -> bool {
        self.e_unset_by_name(feature_id)
    }
}
