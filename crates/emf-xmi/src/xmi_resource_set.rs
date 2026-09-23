//! XMI resource set (port of C++ `emf::xmi::XMIResourceSet`, aligned to Java
//! `org.eclipse.emf.ecore.resource.impl.ResourceSetImpl` + the XMI flavour).
//!
//! A set is the bridge for *resolution*: it owns a group of [`XMIResource`]s
//! bound to one [`PackageRegistry`], lets callers create/find them by URI, and
//! can turn a proxy URI back into the concrete root object it points at
//! (EMF `ResourceSet.getEObject(uri, loadOnDemand)` / `EcoreUtil.resolve`).
//!
//! A resource is shared via `Rc<RefCell<_>>` so the set and the caller hold the
//! same object; the set keeps every resource alive for the whole set lifetime.

use std::cell::RefCell;
use std::rc::Rc;

use emf_common::uri::Uri;
use emf_common::value::ObjectRef;
use emf_ecore::PackageRegistry;

use super::xmi_resource::XMIResource;

/// A collection of XMI resources that can create, find, and resolve objects.
#[derive(Debug, Default)]
pub struct XMIResourceSet {
    registry: PackageRegistry,
    resources: Vec<Rc<RefCell<XMIResource>>>,
}

impl XMIResourceSet {
    /// New empty set whose resources resolve their metamodel via `registry`.
    pub fn new(registry: PackageRegistry) -> Self {
        Self {
            registry,
            resources: Vec::new(),
        }
    }

    /// The registry every resource in this set resolves classes/features against.
    pub fn registry(&self) -> &PackageRegistry {
        &self.registry
    }

    /// All resources in the set, in creation order.
    pub fn resources(&self) -> &[Rc<RefCell<XMIResource>>] {
        &self.resources
    }

    /// EMF `createResource(uri)`: return the existing resource at `uri`, or
    /// create and store a new one. The returned handle is shared with the set.
    pub fn create_resource(&mut self, uri: Uri) -> Rc<RefCell<XMIResource>> {
        if let Some(existing) = self.find_by_uri(&uri) {
            return existing;
        }
        let res = Rc::new(RefCell::new(XMIResource::new(
            uri,
            self.registry.clone(),
        )));
        self.resources.push(Rc::clone(&res));
        res
    }

    /// EMF `getResource(uri, loadOnDemand)`: the resource at `uri`, if present.
    /// With no file scheme nothing is demand-loaded here (in-memory URIs), so
    /// `load_on_demand` is accepted but only the in-set lookup applies.
    pub fn get_resource(&self, uri: &Uri, _load_on_demand: bool) -> Option<Rc<RefCell<XMIResource>>> {
        self.find_by_uri(uri)
    }

    /// EMF `getEObject(uri, loadOnDemand)`: resolve `uri` to a root object of
    /// the resource it addresses, keyed on the URI without its fragment.
    /// Returns `None` when no matching resource exists or it has no roots.
    pub fn get_eobject(&self, uri: &Uri, _load_on_demand: bool) -> Option<ObjectRef> {
        let base = uri.trim_fragment();
        let res = self.find_by_uri(&base)?;
        let root = res.borrow().resource().root().cloned();
        root
    }

    /// Resolve a proxy URI to the concrete root object it points at
    /// (EMF `EcoreUtil.resolve(proxy, resourceSet)`). Returns `None` when the
    /// addressed resource is not in this set or has no roots.
    pub fn resolve_proxy_uri(&self, uri: &Uri) -> Option<ObjectRef> {
        self.get_eobject(uri, true)
    }

    /// Linear scan for a resource whose URI equals `uri` (URI equality ignores
    /// the fragment when comparing).
    fn find_by_uri(&self, uri: &Uri) -> Option<Rc<RefCell<XMIResource>>> {
        self.resources
            .iter()
            .find(|r| r.borrow().resource().uri() == uri)
            .cloned()
    }
}