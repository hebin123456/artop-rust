//! `DynamicEObject` — a reflective object backed by an `EClass` descriptor.
//!
//! Port of C++ `emf-ecore/DynamicEObject` (aligned to Java
//! `org.eclipse.emf.ecore.impl.DynamicEObjectImpl`). Values are stored in a
//! `HashMap<FeatureID, Val>` plus a set of "set" flags so `eIsSet` and
//! `eUnset` behave like EMF: unsetting restores the class's default value.

use crate::{EClass, Val};
use emf_common::eobject::{EObject, InverseList};
use emf_common::notification::{emit, Adapter, EventType, Notification, Notifier, NotifierHandle};
use emf_common::uri::Uri;
use emf_common::value::ObjectRef;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// A back-link from a contained object to its container: a weak handle to the
/// parent plus the name of the containment feature that owns this object.
/// Weak avoids reference cycles between parent and child.
pub type ContainerBackref = (Weak<RefCell<dyn EObject>>, String);

/// Registry of reverse-reference lists keyed by feature id (C++
/// `impl::BasicEObject::eInverseELists_`). A list registers itself on the owner
/// object; the registry holds shared handles and compares them by identity on
/// unregistration, mirroring the C++ raw `EInverseList*` comparison.
#[derive(Default)]
struct InverseLists {
    lists: HashMap<i32, Rc<RefCell<dyn InverseList>>>,
}

impl InverseLists {
    fn register(&mut self, feature_id: i32, list: Rc<RefCell<dyn InverseList>>) {
        self.lists.insert(feature_id, list);
    }

    fn unregister(&mut self, feature_id: i32, list: &Rc<RefCell<dyn InverseList>>) {
        // Only drop the entry when it *is* the list being unregistered; a
        // mismatched list is a no-op (C++ `it->second == list`).
        if let Some(registered) = self.lists.get(&feature_id) {
            if Rc::ptr_eq(registered, list) {
                self.lists.remove(&feature_id);
            }
        }
    }

    fn get(&self, feature_id: i32) -> Option<Rc<RefCell<dyn InverseList>>> {
        self.lists.get(&feature_id).cloned()
    }
}

impl std::fmt::Debug for InverseLists {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InverseLists")
            .field("len", &self.lists.len())
            .finish()
    }
}

#[derive(Debug)]
pub struct DynamicEObject {
    /// The class descriptor (owned clone).
    pub e_class: EClass,
    /// Multi-purpose storage keyed by feature *name*.
    ///
    /// Keyed by name rather than feature id because feature ids are only unique
    /// within a single EPackage: a cross-package subclass (whose supertype
    /// lives in another package that reboots the 0-based numbering) would
    /// otherwise collide — e.g. base's inherited `name` and ext's own `note`
    /// can share id 0, so an id-keyed map would clobber one with the other.
    pub dynamic_settings: HashMap<String, Val>,
    /// Whether each feature (by name) has been set (for `eIsSet`).
    pub set_flags: std::collections::HashSet<String>,
    /// The container (weak parent + containment feature name), if this object is
    /// owned by another object through a containment reference.
    container: Option<ContainerBackref>,
    /// Reverse-reference lists registered by feature id (C++ `BasicEObject`).
    inverse_lists: InverseLists,
    /// Registry snapshot used to resolve the inheritance graph.
    registry: Option<crate::package::PackageRegistry>,
    /// A proxy URI, when this object stands in for an unresolved reference
    /// (`e_is_proxy` == true). `None` for a concrete object.
    proxy_uri: Option<Uri>,
    /// The notification sink for this object (EMF `Notifier`). Adapters
    /// attached here receive SET/UNSET notifications when features change.
    notifier: NotifierHandle,
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
            inverse_lists: InverseLists::default(),
            registry: None,
            proxy_uri: None,
            notifier: new_notifier_handle(),
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
            inverse_lists: InverseLists::default(),
            registry: Some(registry),
            proxy_uri: None,
            notifier: new_notifier_handle(),
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

    /// The notification sink (EMF `Notifier`) for this object.
    pub fn notifier(&self) -> &NotifierHandle {
        &self.notifier
    }

    /// Attach an adapter to receive change notifications (EMF `eAdapters().add`).
    pub fn add_adapter(&self, adapter: Box<dyn Adapter>) {
        self.notifier.borrow_mut().add_adapter(adapter);
    }

    /// Detach adapter(s) matching a predicate (EMF `eAdapters().remove`).
    pub fn remove_adapter(&self, predicate: impl FnMut(&dyn Adapter) -> bool) {
        self.notifier.borrow_mut().remove_adapter(predicate);
    }

    /// Number of attached adapters (EMF `eAdapters().size`).
    pub fn adapter_count(&self) -> usize {
        self.notifier.borrow().adapters().len()
    }

    /// Whether changes must be delivered to adapters (C++
    /// `BasicEObject::eNotificationRequired`).
    pub fn e_notification_required(&self) -> bool {
        self.notifier.borrow().e_notification_required()
    }

    /// Register a reverse-reference list under `feature_id` (C++
    /// `BasicEObject::eRegisterInverseList`). `None` models a null list and is a
    /// no-op.
    pub fn e_register_inverse_list(
        &mut self,
        feature_id: i32,
        list: Option<Rc<RefCell<dyn InverseList>>>,
    ) {
        if let Some(list) = list {
            self.inverse_lists.register(feature_id, list);
        }
    }

    /// Unregister a reverse-reference list (C++
    /// `BasicEObject::eUnregisterInverseList`). Only removes the entry when it
    /// is exactly `list`; a mismatched list is a no-op.
    pub fn e_unregister_inverse_list(
        &mut self,
        feature_id: i32,
        list: &Rc<RefCell<dyn InverseList>>,
    ) {
        self.inverse_lists.unregister(feature_id, list);
    }

    /// The name of the structural feature with `feature_id`, if resolvable
    /// (C++ `eClass()->getEStructuralFeature(featureID)`); used to label the
    /// reverse notifications emitted by [`EObject::e_inverse_add`].
    fn feature_name_by_id(&self, feature_id: i32) -> Option<String> {
        if feature_id < 0 {
            return None;
        }
        self.e_all()
            .into_iter()
            .find(|f| f.feature_id() == feature_id)
            .map(|f| f.name().to_string())
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
        let name = feature.name();
        if let Some(v) = self.dynamic_settings.get(name) {
            return v.clone();
        }
        // An unset many-valued reference reads as an empty list (EMF).
        if feature.upper_bound() == -1 && feature.is_reference() {
            return Val::List(Vec::new());
        }
        feature.default_value().cloned().unwrap_or(Val::Null)
    }

    /// Set a feature by name. Returns false if the feature name is unknown.
    pub fn e_set_by_name(&mut self, name: &str, value: Val) -> bool {
        let feature = match self.e_all().into_iter().find(|f| f.name() == name) {
            Some(f) => f,
            None => return false,
        };
        // Reference features store object refs / object lists; attributes store
        // atomic values.
        let stored = if feature.is_reference() {
            normalize_reference_value(&value)
        } else {
            value
        };
        // Emit a SET notification for single-valued features only, mirroring the
        // C++ `DynamicEObject::eSet` (multi-valued writes go through the list and
        // return before notifying). Old value is the raw stored value (Null when
        // previously unset), matching C++.
        let notify = feature.upper_bound() != -1;
        let old_value = if notify {
            self.dynamic_settings
                .get(name)
                .cloned()
                .unwrap_or(Val::Null)
        } else {
            Val::Null
        };
        self.dynamic_settings
            .insert(name.to_string(), stored.clone());
        self.set_flags.insert(name.to_string());
        if notify {
            let n = Notification::new(
                EventType::Set,
                Some(name.to_string()),
                old_value,
                stored,
                -1,
                false,
            );
            emit(&self.notifier, &n);
        }
        true
    }

    /// Whether a feature (by name) is set.
    pub fn e_is_set_by_name(&self, name: &str) -> Option<bool> {
        let feature = self.e_all().into_iter().find(|f| f.name() == name)?;
        // A multi-valued feature is "set" if it has been touched (flag) and is
        // non-empty; a single-valued feature follows the flag alone.
        let flag = self.set_flags.contains(name);
        if feature.upper_bound() == -1 {
            let non_empty =
                matches!(self.dynamic_settings.get(name), Some(Val::List(l)) if !l.is_empty());
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
        // Detach contained children before clearing.
        if feature.is_containment() {
            clear_container_children(self, name);
        }
        // Capture the raw old value before clearing for the UNSET notification
        // (mirrors C++ `DynamicEObject::eUnset`).
        let old_value = self
            .dynamic_settings
            .get(name)
            .cloned()
            .unwrap_or(Val::Null);
        self.dynamic_settings.remove(name);
        self.set_flags.remove(name);
        let n = Notification::new(
            EventType::Unset,
            Some(name.to_string()),
            old_value,
            Val::Null,
            -1,
            false,
        );
        emit(&self.notifier, &n);
        true
    }

    // ---- container / containment graph ----

    /// Set (or clear with `None`) the weak back-link to this object's container.
    /// Called by a parent when it adopts this object through a containment ref.
    /// Internal: does not notify (C++ `eSetContainer`).
    pub fn set_container(&mut self, container: Option<ContainerBackref>) {
        self.container = container;
    }

    /// Set (or clear with `None`) this object's container, firing the reverse
    /// REMOVE(old)/ADD(new) notifications (C++ `EObjectImpl::setEContainer`).
    /// Assigning the same container is a no-op.
    pub fn set_e_container(&mut self, container: Option<ObjectRef>) {
        let same = match (&self.container, &container) {
            (None, None) => true,
            (Some((weak, _)), Some(new)) => weak.upgrade().is_some_and(|old| Rc::ptr_eq(&old, new)),
            _ => false,
        };
        if same {
            return;
        }
        let notify = self.e_notification_required();
        let old = self.container.take().and_then(|(weak, _)| weak.upgrade());
        // No containment feature is recorded here (C++ setEContainer does not
        // touch eContainingFeature), so the back-link keeps an empty feature.
        self.container = container
            .as_ref()
            .map(|c| (Rc::downgrade(c), String::new()));
        if !notify {
            return;
        }
        if let Some(old) = old {
            let r = Notification::new(
                EventType::Remove,
                None,
                Val::Object(old),
                Val::Null,
                -1,
                false,
            );
            emit(&self.notifier, &r);
        }
        if let Some(new) = container {
            let a = Notification::new(EventType::Add, None, Val::Null, Val::Object(new), -1, false);
            emit(&self.notifier, &a);
        }
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
            let name = f.name();
            match self.dynamic_settings.get(name) {
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
        if !self.e_all().into_iter().any(|f| f.name() == name) {
            return Vec::new();
        }
        let cur = self
            .dynamic_settings
            .get(name)
            .cloned()
            .unwrap_or(Val::Null);
        let objs = cur
            .as_list()
            .map(|l| l.iter().filter_map(|v| v.as_object().cloned()).collect())
            .unwrap_or_default();
        self.set_flags.insert(name.to_string());
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
        self.container.as_ref().and_then(|(weak, _)| weak.upgrade())
    }

    fn e_containing_feature(&self) -> Option<String> {
        self.container.as_ref().and_then(|(weak, feature)| {
            weak.upgrade().and_then(|_| {
                // An empty feature marks a container set via `set_e_container`
                // (no containment feature recorded) -> report none, matching C++.
                if feature.is_empty() {
                    None
                } else {
                    Some(feature.clone())
                }
            })
        })
    }

    fn set_e_container(&mut self, container: Option<ObjectRef>) {
        DynamicEObject::set_e_container(self, container);
    }

    fn e_notification_required(&self) -> bool {
        DynamicEObject::e_notification_required(self)
    }

    fn e_inverse_add(
        &self,
        other_end: &ObjectRef,
        feature_id: i32,
        mut notifications: Vec<Notification>,
    ) -> Vec<Notification> {
        if let Some(list) = self.inverse_lists.get(feature_id) {
            list.borrow_mut().basic_add(other_end);
            if self.e_notification_required() {
                notifications.push(Notification::new(
                    EventType::Add,
                    self.feature_name_by_id(feature_id),
                    Val::Null,
                    Val::Object(Rc::clone(other_end)),
                    -1,
                    false,
                ));
            }
        }
        notifications
    }

    fn e_inverse_remove(
        &self,
        other_end: &ObjectRef,
        feature_id: i32,
        mut notifications: Vec<Notification>,
    ) -> Vec<Notification> {
        if let Some(list) = self.inverse_lists.get(feature_id) {
            list.borrow_mut().basic_remove(other_end);
            if self.e_notification_required() {
                notifications.push(Notification::new(
                    EventType::Remove,
                    self.feature_name_by_id(feature_id),
                    Val::Object(Rc::clone(other_end)),
                    Val::Null,
                    -1,
                    false,
                ));
            }
        }
        notifications
    }

    fn e_contents(&self) -> Vec<ObjectRef> {
        self.contents()
    }

    fn e_cross_references(&self) -> Vec<ObjectRef> {
        // Non-containment references' targets (EMF `eCrossReferences`), used by
        // reflective proxy checks (e.g. AUTOSAR no-unresolved-proxy). Only the
        // target objects held by *non-containment* reference features are
        // returned; containment children are already surfaced via `e_contents`.
        let mut out = Vec::new();
        for f in self.all_references() {
            if f.is_containment() {
                continue;
            }
            match self.dynamic_settings.get(f.name()) {
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

    fn e_proxy_uri(&self) -> Option<&Uri> {
        self.proxy_uri.as_ref()
    }

    fn e_set_proxy_uri(&mut self, uri: Option<Uri>) {
        self.proxy_uri = uri;
    }

    fn e_is_proxy(&self) -> bool {
        self.proxy_uri.is_some()
    }

    fn e_resolve_proxy(&self, proxy: &ObjectRef) -> ObjectRef {
        // Base contract (C++ `eResolveProxy`): a non-proxy resolves to itself.
        // When `proxy` is a real proxy with no resolving ResourceSet handy, the
        // degenerate result is the proxy unchanged. The ResourceSet-level
        // demand-load resolution lives on the XMI resource set (EMF `EcoreUtil`).
        Rc::clone(proxy)
    }
}

/// A fresh notifier handle with delivery enabled (EMF `Notifier` default).
fn new_notifier_handle() -> NotifierHandle {
    Rc::new(RefCell::new(Notifier::new()))
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

/// Detach the contained children of `name` from their container back-link.
fn clear_container_children(obj: &mut DynamicEObject, name: &str) {
    match obj.dynamic_settings.get(name).cloned() {
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
    parent
        .borrow_mut()
        .e_set_by_name(name, Val::Object(node_to_object(child)));
    child
        .borrow_mut()
        .set_container(Some((weak, name.to_string())));
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
    child
        .borrow_mut()
        .set_container(Some((weak, name.to_string())));
}
