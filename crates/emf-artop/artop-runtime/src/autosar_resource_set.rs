//! AUTOSAR resource set (port of C++ `AutosarResourceSet`, aligned to Java
//! `org.artop.aal.common.resource.impl.AutosarResourceSetImpl` +
//! `org.eclipse.emf.ecore.resource.impl.ResourceSetImpl`).
//!
//! Responsibilities (mirroring the C++ header):
//!   1. create and hold multiple [`AutosarResource`]s (allocated by URI);
//!   2. find an already-loaded resource by URI, or demand-load it;
//!   3. register a loaded resource's shortName paths into the global
//!      [`AutosarLibraryIndex`] so cross-document references resolve;
//!   4. resolve a `URI#fragment` to an object across resources.
//!
//! Unlike [`emf_xmi::XMIResourceSet`] this set produces AUTOSAR resources and
//! indexes them into the library on load.

use std::cell::RefCell;
use std::rc::Rc;

use emf_common::uri::Uri;
use emf_common::value::ObjectRef;
use emf_ecore::PackageRegistry;

use crate::autosar_resource::AutosarResource;
use crate::release_descriptor::AutosarReleaseDescriptor;

/// A collection of [`AutosarResource`]s that can create, find, demand-load and
/// index them.
#[derive(Default)]
pub struct AutosarResourceSet {
    registry: PackageRegistry,
    release: Option<AutosarReleaseDescriptor>,
    resources: Vec<Rc<RefCell<AutosarResource>>>,
}

impl AutosarResourceSet {
    /// New empty set whose resources resolve their metamodel via `registry`.
    pub fn new(registry: PackageRegistry) -> Self {
        Self {
            registry,
            release: None,
            resources: Vec::new(),
        }
    }

    /// New empty set carrying an AUTOSAR `release` descriptor.
    pub fn with_release(registry: PackageRegistry, release: AutosarReleaseDescriptor) -> Self {
        Self {
            registry,
            release: Some(release),
            resources: Vec::new(),
        }
    }

    /// The registry every resource in this set resolves against.
    pub fn registry(&self) -> &PackageRegistry {
        &self.registry
    }

    /// All resources in the set, in creation order.
    pub fn resources(&self) -> &[Rc<RefCell<AutosarResource>>] {
        &self.resources
    }

    /// EMF `createResource(uri)`: return the existing resource at `uri`, or
    /// create and store a new one (C++ `AutosarResourceSet::createResource`).
    pub fn create_resource(&mut self, uri: Uri) -> Rc<RefCell<AutosarResource>> {
        if let Some(existing) = self.find_by_uri(&uri) {
            return existing;
        }
        let res = Rc::new(RefCell::new(AutosarResource::with_release(
            uri,
            self.registry.clone(),
            self.release.clone(),
        )));
        self.resources.push(Rc::clone(&res));
        res
    }

    /// EMF `getResource(uri, loadOnDemand)`: the resource at `uri`, loading it
    /// first when `load_on_demand` and absent (C++ `AutosarResourceSet::
    /// getResource`). A load failure surfaces as `Err`.
    pub fn get_resource(
        &mut self,
        uri: &Uri,
        load_on_demand: bool,
    ) -> Result<Option<Rc<RefCell<AutosarResource>>>, String> {
        if let Some(existing) = self.find_by_uri(uri) {
            if load_on_demand && !existing.borrow().resource().is_loaded() {
                existing.borrow_mut().load()?;
                self.index_library(&existing);
            }
            return Ok(Some(existing));
        }
        if !load_on_demand {
            return Ok(None);
        }
        let res = self.create_resource(uri.clone());
        res.borrow_mut().load()?;
        self.index_library(&res);
        Ok(Some(res))
    }

    /// Convenience: preload a library resource (load + index), C++
    /// `AutosarResourceSet::loadLibrary`.
    pub fn load_library(
        &mut self,
        uri: &Uri,
    ) -> Result<Option<Rc<RefCell<AutosarResource>>>, String> {
        self.get_resource(uri, true)
    }

    /// Resolve a `URI#fragment` to a root's descendant across resources (C++
    /// `AutosarResourceSet::getEObject`). The fragment is resolved against the
    /// resource addressed by the URI without its fragment.
    pub fn get_eobject(&self, uri: &Uri, _load_on_demand: bool) -> Option<ObjectRef> {
        let base = uri.trim_fragment();
        let res = self.find_by_uri(&base)?;
        let fragment = uri.fragment();
        let found = res.borrow().get_eobject(fragment);
        found
    }

    /// Register `res`'s shortName paths into the global library index
    /// (C++ `AutosarResourceSet::indexLibrary`).
    fn index_library(&self, res: &Rc<RefCell<AutosarResource>>) {
        res.borrow().index_library();
    }

    /// Linear scan for a resource whose URI equals `uri` (URI equality ignores
    /// the fragment).
    fn find_by_uri(&self, uri: &Uri) -> Option<Rc<RefCell<AutosarResource>>> {
        self.resources
            .iter()
            .find(|r| r.borrow().resource().uri() == uri)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version_data::AutosarMetaModelVersionData;

    #[test]
    fn create_and_find_by_uri() {
        let mut set = AutosarResourceSet::new(PackageRegistry::default());
        let uri = Uri::parse("file:///tmp/a.arxml");
        let r1 = set.create_resource(uri.clone());
        let r2 = set.create_resource(uri.clone());
        assert!(Rc::ptr_eq(&r1, &r2));
        assert_eq!(set.resources().len(), 1);
    }

    #[test]
    fn carries_release_descriptor() {
        let release = AutosarReleaseDescriptor::new(
            "org.artop.aal.autosar448".to_string(),
            AutosarMetaModelVersionData::new(4, 4, 8),
        );
        let mut set = AutosarResourceSet::with_release(PackageRegistry::default(), release);
        let r = set.create_resource(Uri::parse("file:///tmp/a.arxml"));
        assert!(r.borrow().autosar_release().is_some());
    }

    #[test]
    fn get_resource_absent_without_demand_is_none() {
        let mut set = AutosarResourceSet::new(PackageRegistry::default());
        let got = set
            .get_resource(&Uri::parse("file:///tmp/missing.arxml"), false)
            .unwrap();
        assert!(got.is_none());
        assert_eq!(set.resources().len(), 0);
    }
}
