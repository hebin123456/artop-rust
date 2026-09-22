//! XMI resource factory (port of C++ `emf::xmi::XMIResourceFactory`, aligned to
//! Java `org.eclipse.emf.ecore.xmi.impl.XMIResourceFactoryImpl`).
//!
//! A [`ResourceFactory`] that produces [`XMIResource`]s bound to a shared
//! [`PackageRegistry`]. Register it on a [`ResourceSet`] (via
//! `ResourceSet::set_resource_factory`) and `getResource(uri, true)` will
//! create + load an XMI-backed resource on demand — without this crate ever
//! coupling back into `emf-common`.

use emf_common::resource::{ResourceFactory, ResourceHandle};
use emf_common::uri::Uri;
use emf_ecore::PackageRegistry;

use super::xmi_resource::XMIResource;

/// Creates [`XMIResource`]s resolving their metamodel via `registry`.
#[derive(Debug, Clone)]
pub struct XMIResourceFactory {
    registry: PackageRegistry,
}

impl XMIResourceFactory {
    /// A factory producing XMI resources for the given registry.
    pub fn new(registry: PackageRegistry) -> Self {
        Self { registry }
    }

    /// The registry every produced resource resolves classes/features against.
    pub fn registry(&self) -> &PackageRegistry {
        &self.registry
    }
}

impl ResourceFactory for XMIResourceFactory {
    fn create(&self, uri: Uri) -> Box<dyn ResourceHandle> {
        Box::new(XMIResource::new(uri, self.registry.clone()))
    }
}
