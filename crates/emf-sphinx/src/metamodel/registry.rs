//! `MetaModelDescriptorRegistry` — namespace-indexed descriptor registry.
//!
//! Port of C++ `emf/sphinx/metamodel/MetaModelDescriptorRegistry.h` (aligned to
//! Java `org.eclipse.sphinx.emf.metamodel.MetaModelDescriptorRegistry`).
//!
//! C++ uses a process-wide singleton. Rust keeps the state in a thread-local so
//! parallel tests stay isolated, and exposes the same operations through
//! [`MetaModelDescriptorRegistry::instance`].

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use emf_common::eobject::EObject;
use emf_common::resource::Resource;
use emf_common::uri::Uri;

use super::descriptor::MetaModelDescriptor;

#[derive(Default)]
struct RegistryState {
    descriptors: BTreeMap<String, Rc<dyn MetaModelDescriptor>>,
    target_descriptors: BTreeMap<String, Rc<dyn MetaModelDescriptor>>,
    old_descriptors: BTreeMap<String, Rc<dyn MetaModelDescriptor>>,
}

thread_local! {
    static STATE: RefCell<RegistryState> = RefCell::new(RegistryState::default());
}

/// Namespace-indexed registry of meta-model descriptors.
#[derive(Debug, Clone, Copy, Default)]
pub struct MetaModelDescriptorRegistry;

impl MetaModelDescriptorRegistry {
    /// The singleton registry handle (aligned to C++ `instance()`).
    pub fn instance() -> Self {
        MetaModelDescriptorRegistry
    }

    /// Register a descriptor under its namespace. Empty namespaces are ignored;
    /// re-registering the same namespace replaces the previous entry.
    pub fn register_descriptor(&self, d: Rc<dyn MetaModelDescriptor>) {
        let ns = d.namespace();
        if ns.is_empty() {
            return;
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.target_descriptors.insert(ns.clone(), Rc::clone(&d));
            s.old_descriptors.insert(ns.clone(), Rc::clone(&d));
            s.descriptors.insert(ns, d);
        });
    }

    /// Unregister a descriptor (matched by handle identity).
    pub fn unregister_descriptor(&self, d: &Rc<dyn MetaModelDescriptor>) {
        let ns = d.namespace();
        if ns.is_empty() {
            return;
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if s.descriptors
                .get(&ns)
                .is_some_and(|stored| Rc::ptr_eq(stored, d))
            {
                s.descriptors.remove(&ns);
            }
            if s.target_descriptors
                .get(&ns)
                .is_some_and(|stored| Rc::ptr_eq(stored, d))
            {
                s.target_descriptors.remove(&ns);
            }
            if s.old_descriptors
                .get(&ns)
                .is_some_and(|stored| Rc::ptr_eq(stored, d))
            {
                s.old_descriptors.remove(&ns);
            }
        });
    }

    /// Look up by namespace URI string.
    pub fn get_descriptor(&self, ns_uri: &str) -> Option<Rc<dyn MetaModelDescriptor>> {
        STATE.with(|s| s.borrow().descriptors.get(ns_uri).cloned())
    }

    /// Look up by namespace [`Uri`].
    pub fn get_descriptor_uri(&self, uri: &Uri) -> Option<Rc<dyn MetaModelDescriptor>> {
        self.get_descriptor(&uri.to_string())
    }

    /// Look up the descriptor for an `EObject` via its package namespace.
    ///
    /// The headless Rust object graph carries no `EPackage` back-link, so only
    /// the null case is defined (returns `None`); the C++ implementation walks
    /// `eClass().getEPackage().getNsURI()`.
    pub fn get_descriptor_for_object(
        &self,
        obj: Option<&dyn EObject>,
    ) -> Option<Rc<dyn MetaModelDescriptor>> {
        let _ = obj;
        None
    }

    /// Look up the descriptor for a resource via its first content object.
    pub fn get_descriptor_for_resource(
        &self,
        res: Option<&Resource>,
    ) -> Option<Rc<dyn MetaModelDescriptor>> {
        let res = res?;
        if res.contents().is_empty() {
            return None;
        }
        // C++ resolves the first content object's package namespace; the
        // headless Rust object graph has no such back-link.
        None
    }

    /// Look up a target descriptor by namespace [`Uri`].
    pub fn get_target_descriptor(&self, uri: &Uri) -> Option<Rc<dyn MetaModelDescriptor>> {
        STATE.with(|s| s.borrow().target_descriptors.get(&uri.to_string()).cloned())
    }

    /// Look up an old descriptor by namespace [`Uri`].
    pub fn get_old_descriptor(&self, uri: &Uri) -> Option<Rc<dyn MetaModelDescriptor>> {
        STATE.with(|s| s.borrow().old_descriptors.get(&uri.to_string()).cloned())
    }

    /// All registered namespace keys.
    pub fn keys(&self) -> Vec<String> {
        STATE.with(|s| s.borrow().descriptors.keys().cloned().collect())
    }

    /// All registered descriptors.
    pub fn all(&self) -> Vec<Rc<dyn MetaModelDescriptor>> {
        STATE.with(|s| s.borrow().descriptors.values().cloned().collect())
    }

    /// Remove every registration.
    pub fn clear(&self) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.descriptors.clear();
            s.target_descriptors.clear();
            s.old_descriptors.clear();
        });
    }
}
