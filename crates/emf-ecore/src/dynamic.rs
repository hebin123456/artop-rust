//! `DynamicEObject` — a reflective object backed by an `EClass` descriptor.
//!
//! Port of C++ `emf-ecore/DynamicEObject` (aligned to Java
//! `org.eclipse.emf.ecore.impl.DynamicEObjectImpl`). Values are stored in a
//! `HashMap<FeatureID, Val>` plus a set of "set" flags so `eIsSet` and
//! `eUnset` behave like EMF: unsetting restores the class's default value.

use crate::{EClass, Val};
use emf_common::eobject::EObject;
use emf_common::value::ObjectRef;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// A back-link from a contained object to its container: a weak handle to the
/// parent plus the name of the containment feature that owns this object.
/// Weak avoids reference cycles between parent and child.
pub type ContainerBackref = (Weak<RefCell<dyn EObject>>, String);

#[derive(Debug)]
pub struct DynamicEObject {
    /// The class descriptor (owned clone).
    pub e_class: EClass,
    /// Multi-purpose storage keyed by feature id.
    pub dynamic_settings: HashMap<i32, Val>,
    /// Whether each feature id has been set (for `eIsSet`).
    pub set_flags: std::collections::HashSet<i32>,
    /// The container (weak parent + containment feature name), if this object is
    /// owned by another object through a containment reference.
    container: Option<ContainerBackref>,
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
            container: None,
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
            container: None,
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
            // An unset many-valued feature reads as an empty list (EMF).
            if feature.upper_bound() == -1 && feature.is_reference() {
                return Val::List(Vec::new());
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
        if id < 0 {
            return true;
        }
        // Reference features store object refs / object lists; attributes store
        // atomic values.
        if feature.is_reference() {
            self.dynamic_settings.insert(id, normalize_reference_value(&value));
        } else {
            self.dynamic_settings.insert(id, value);
        }
        self.set_flags.insert(id);
        true
    }

    /// Whether a feature (by name) is set.
    pub fn e_is_set_by_name(&self, name: &str) -> Option<bool> {
        let feature = self.e_all().into_iter().find(|f| f.name() == name)?;
        let id = feature.feature_id();
        // A multi-valued feature is "set" if it has been touched (flag) and is
        // non-empty; a single-valued feature follows the flag alone.
        let flag = self.set_flags.contains(&id);
        if feature.upper_bound() == -1 {
            let non_empty = matches!(self.dynamic_settings.get(&id), Some(Val::List(l)) if !l.is_empty());
            Some(flag && non_empty)
        } else {
            Some(flag)
        }
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
            // Detach contained children before clearing.
            if feature.is_containment() {
                clear_container_children(self, id);
            }
            self.dynamic_settings.remove(&id);
            self.set_flags.remove(&id);
        }
        true
    }

    // ---- container / containment graph ----

    /// Set (or clear with `None`) the weak back-link to this object's container.
    /// Called by a parent when it adopts this object through a containment ref.
    pub fn set_container(&mut self, container: Option<ContainerBackref>) {
        self.container = container;
    }

    /// The weak back-link to this object's container.
    pub fn container_backref(&self) -> Option<&ContainerBackref> {
        self.container.as_ref()
    }

    /// The containment children held by single and multi containment features,
    /// in feature-id order (mirrors C++ `eContents`).
    pub fn contents(&self) -> Vec<ObjectRef> {
        let mut out = Vec::new();
        for f in self.all_containments() {
            let id = f.feature_id();
            match self.dynamic_settings.get(&id) {
                Some(Val::Object(o)) => out.push(o.clone()),
                Some(Val::List(l)) => {
                    for e in l {
                        if let Some(o) = e.as_object() {
                            out.push(o.clone());
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// The multi-valued list for a feature, expanding an unset default to an
    /// empty list so callers can populate it (mirrors C++ lazy list creation).
    pub fn e_list_mut(&mut self, name: &str) -> Vec<ObjectRef> {
        let feature = match self.e_all().into_iter().find(|f| f.name() == name) {
            Some(f) => f,
            None => return Vec::new(),
        };
        let id = feature.feature_id();
        let cur = self.dynamic_settings.get(&id).cloned().unwrap_or(Val::Null);
        let objs = cur
            .as_list()
            .map(|l| l.iter().filter_map(|v| v.as_object().cloned()).collect())
            .unwrap_or_default();
        self.set_flags.insert(id);
        objs
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

    fn clear_container(&mut self) {
        self.container = None;
    }

    fn e_container(&self) -> Option<ObjectRef> {
        self.container
            .as_ref()
            .and_then(|(weak, _)| weak.upgrade())
    }

    fn e_contents(&self) -> Vec<ObjectRef> {
        self.contents()
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

/// Normalize a caller-supplied reference value so it is stored as a proper
/// reference: single objects stay `Val::Object`, lists become `Val::List`.
fn normalize_reference_value(value: &Val) -> Val {
    match value {
        Val::List(_) => {
            let objs = value
                .as_list()
                .map(|l| {
                    l.iter()
                        .filter_map(|v| v.as_object().cloned())
                        .map(Val::Object)
                        .collect()
                })
                .unwrap_or_default();
            Val::List(objs)
        }
        other => other.clone(),
    }
}

/// Detach the contained children of `id` from their container back-link.
fn clear_container_children(obj: &mut DynamicEObject, id: i32) {
    match obj.dynamic_settings.get(&id).cloned() {
        Some(Val::Object(o)) => {
            o.borrow_mut().clear_container();
        }
        Some(Val::List(l)) => {
            let objs: Vec<ObjectRef> = l.iter().filter_map(|v| v.as_object().cloned()).collect();
            for o in objs {
                o.borrow_mut().clear_container();
            }
        }
        _ => {}
    }
}

/// A shared handle to a `DynamicEObject` for graph building (containment).
pub type DynNode = Rc<RefCell<DynamicEObject>>;

/// Coerce a `DynNode` to a type-erased `ObjectRef` (`dyn EObject`).
pub fn node_to_object(node: &DynNode) -> ObjectRef {
    node.clone() as ObjectRef
}

/// Adopt `child` into `parent` through a single containment feature `name`:
/// records the value on the parent and sets the child's weak container back-link.
pub fn adopt_single(parent: &DynNode, name: &str, child: &DynNode) {
    let weak = Rc::downgrade(&(parent.clone() as ObjectRef));
    parent.borrow_mut().e_set_by_name(name, Val::Object(node_to_object(child)));
    child.borrow_mut().set_container(Some((weak, name.to_string())));
}

/// Adopt `child` into `parent` through a multi containment feature `name`,
/// appending to the stored list and setting the child's container back-link.
pub fn adopt_many(parent: &DynNode, name: &str, child: &DynNode) {
    let weak = Rc::downgrade(&(parent.clone() as ObjectRef));
    {
        let mut p = parent.borrow_mut();
        let mut list = p.e_list_mut(name);
        list.push(node_to_object(child));
        p.e_set_by_name(name, Val::List(list.into_iter().map(Val::Object).collect()));
    }
    child.borrow_mut().set_container(Some((weak, name.to_string())));
}
