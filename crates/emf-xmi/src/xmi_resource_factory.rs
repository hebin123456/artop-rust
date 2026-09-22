//! XMI resource factory (port of C++ `emf::xmi::XMIResourceFactory`, aligned to
//! Java `org.eclipse.emf.ecore.xmi.impl.XMIResourceFactoryImpl`).
//!
//! A [`ResourceFactory`] that produces [`XMIResource`]s bound to a shared
//! [`PackageRegistry`]. Register it on a [`ResourceSet`] (via
//! `ResourceSet::set_resource_factory`) and `getResource(uri, true)` will
//! create + load an XMI-backed resource on demand — without this crate ever
//! coupling back into `emf-common`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use emf_common::resource::{ResourceFactory, ResourceHandle};
use emf_common::uri::Uri;
use emf_ecore::PackageRegistry;

use super::xmi_resource::XMIResource;

/// A resource constructor: `Uri -> resource handle` (C++ lambda-style factory).
type ResourceCtor = Rc<dyn Fn(Uri) -> Box<dyn ResourceHandle>>;

/// The default XMI resource constructor, resolving against an empty registry
/// (the C++ `XMIResourceFactory` default path has no domain metamodel bound).
fn default_xmi_ctor() -> ResourceCtor {
    Rc::new(|uri: Uri| -> Box<dyn ResourceHandle> {
        Box::new(XMIResource::new(uri, PackageRegistry::default()))
    })
}

thread_local! {
    /// Extension (lower-cased, no leading dot) -> resource constructor.
    static FACTORIES: RefCell<HashMap<String, ResourceCtor>> = RefCell::new(HashMap::new());
    /// Fallback used when no extension matches (always a plain XMI resource).
    static FALLBACK: RefCell<ResourceCtor> = RefCell::new(default_xmi_ctor());
}

/// The lower-cased file extension of `uri` (empty when none), without the dot.
fn ext_of(uri: &Uri) -> String {
    uri.path()
        .rsplit('/')
        .next()
        .unwrap_or("")
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase()
}

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

    /// Register the built-in factories so `create_resource_for` dispatches on
    /// the `.xmi` / `.ecore` extensions (EMF `XMIResourceFactory.registerDefaults`).
    pub fn register_defaults() {
        let xmi = default_xmi_ctor();
        FACTORIES.with(|f| {
            let mut m = f.borrow_mut();
            m.insert("xmi".to_string(), xmi.clone());
            m.insert("ecore".to_string(), xmi);
        });
    }

    /// Register (or replace) a custom factory for `extension`, matched
    /// case-insensitively (EMF `XMIResourceFactory.registerFactory`).
    pub fn register_factory<F>(extension: &str, ctor: F)
    where
        F: Fn(Uri) -> Box<dyn ResourceHandle> + 'static,
    {
        let key = extension.trim_start_matches('.').to_lowercase();
        FACTORIES.with(|f| {
            f.borrow_mut().insert(key, Rc::new(ctor));
        });
    }

    /// Create a resource for `uri`, dispatching on its file extension; unknown
    /// extensions fall back to a plain XMI resource (never panics).
    pub fn create_resource_for(uri: Uri) -> Box<dyn ResourceHandle> {
        let ext = ext_of(&uri);
        FACTORIES.with(|f| {
            let m = f.borrow();
            if let Some(ctor) = m.get(&ext) {
                ctor(uri.clone())
            } else {
                FALLBACK.with(|fb| (fb.borrow())(uri))
            }
        })
    }

    /// Directly create an XMI resource for `uri` with no extension dispatch
    /// (EMF `XMIResourceFactory.createResource`).
    pub fn create_resource(uri: Uri) -> Box<dyn ResourceHandle> {
        default_xmi_ctor()(uri)
    }
}

impl ResourceFactory for XMIResourceFactory {
    fn create(&self, uri: Uri) -> Box<dyn ResourceHandle> {
        Box::new(XMIResource::new(uri, self.registry.clone()))
    }
}
