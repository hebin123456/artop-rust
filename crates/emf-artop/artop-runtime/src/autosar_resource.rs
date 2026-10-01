//! AUTOSAR resources (port of C++ `AutosarResource` / `AutosarXMLResource`,
//! aligned to Java `org.artop.aal.common.resource.impl.AutosarResourceImpl` /
//! `AutosarXMLResourceImpl`).
//!
//! Responsibilities (mirroring the C++ headers):
//!   1. carry the [`AutosarReleaseDescriptor`] on top of a generic
//!      [`XMIResource`];
//!   2. expose the arxml-specific `schemaLocation` (the `xsi:schemaLocation`
//!      value an arxml document declares);
//!   3. build the [`XMLHelper`] the serializer/deserializer use;
//!   4. register the resource's shortName paths into the global
//!      [`AutosarLibraryIndex`] so cross-document references can be resolved.
//!
//! Rust has no inheritance, so [`AutosarResource`] *wraps* an [`XMIResource`]
//! and derefs to it; [`AutosarXMLResource`] wraps an [`AutosarResource`] and is
//! the arxml-specific flavour (the C++ `AutosarXMLResource`).

use std::collections::HashSet;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use emf_common::eobject::EObject;
use emf_common::uri::Uri;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::{DynamicEObject, PackageRegistry};
use emf_xmi::{XMIResource, XMLHelper, XMLLoader, XMLSave};

use crate::arxml::{store, AutosarXMLLoader, AutosarXMLSaver};
use crate::autosar_library_index::AutosarLibraryIndex;
use crate::release_descriptor::AutosarReleaseDescriptor;

/// An AUTOSAR-aware XMI resource (C++ `AutosarResource`).
pub struct AutosarResource {
    /// The wrapped generic XMI resource.
    inner: XMIResource,
    /// The associated release descriptor, when known.
    autosar_release: Option<AutosarReleaseDescriptor>,
    /// The arxml `xsi:schemaLocation` value.
    schema_location: String,
}

impl std::fmt::Debug for AutosarResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Delegate to the wrapped XMIResource's summary (the durable
        // uri/loaded/contents shape) plus the AUTOSAR-specific bits.
        f.debug_struct("AutosarResource")
            .field("inner", &self.inner)
            .field("autosar_release", &self.autosar_release)
            .field("schema_location", &self.schema_location)
            .finish()
    }
}

impl AutosarResource {
    /// New resource at `uri` resolving its metamodel via `registry`.
    pub fn new(uri: Uri, registry: PackageRegistry) -> Self {
        Self {
            inner: Self::new_inner(uri, registry),
            autosar_release: None,
            schema_location: String::new(),
        }
    }

    /// New resource carrying an AUTOSAR `release` descriptor (C++
    /// `AutosarResource(uri, release)`).
    pub fn with_release(
        uri: Uri,
        registry: PackageRegistry,
        release: Option<AutosarReleaseDescriptor>,
    ) -> Self {
        Self {
            inner: Self::new_inner(uri, registry),
            autosar_release: release,
            schema_location: String::new(),
        }
    }

    /// Build the wrapped resource with the arxml deserializer and serializer
    /// installed (the C++ `AutosarXMLResource::createXMLLoad` /
    /// `createXMLSave` seams). Rust has no virtual dispatch on the resource, so
    /// both are injected at construction instead of being queried during
    /// `load` / `save`.
    fn new_inner(uri: Uri, registry: PackageRegistry) -> XMIResource {
        let mut inner = XMIResource::new(uri, registry);
        inner.set_xml_load(AutosarXMLLoader::new());
        inner.set_xml_save(AutosarXMLSaver::new());
        inner
    }

    /// The associated release descriptor, if any (C++ `getAutosarRelease`).
    pub fn autosar_release(&self) -> Option<&AutosarReleaseDescriptor> {
        self.autosar_release.as_ref()
    }

    /// Replace the release descriptor (C++ `setAutosarRelease`).
    pub fn set_autosar_release(&mut self, release: Option<AutosarReleaseDescriptor>) {
        self.autosar_release = release;
    }

    /// The arxml `xsi:schemaLocation` value (C++ `getSchemaLocation`).
    pub fn schema_location(&self) -> &str {
        &self.schema_location
    }

    /// Set the arxml `xsi:schemaLocation` value (C++ `setSchemaLocation`).
    pub fn set_schema_location(&mut self, value: impl Into<String>) {
        self.schema_location = value.into();
    }

    /// Build the XML helper for this resource (C++ `createXMLHelper`).
    pub fn create_xml_helper(&self) -> XMLHelper {
        let mut helper = XMLHelper::new();
        helper.set_resource(Some(self.inner.resource().clone()));
        helper
    }

    /// Create the serializer for this resource (C++ `createXMLSave`).
    ///
    /// Returns the dedicated arxml serializer ([`AutosarXMLSaver`]), matching
    /// the C++ `AutosarXMLResource::createXMLSave`.
    pub fn create_xml_save(&self) -> Rc<dyn XMLSave> {
        Rc::new(AutosarXMLSaver::new())
    }

    /// Create the deserializer for this resource (C++ `createXMLLoad`).
    ///
    /// The dedicated arxml deserializer ([`AutosarXMLLoader`]) drives the load:
    /// it maps arxml elements to features through the bridged metamodel and
    /// resolves short-name-path references. Its counterpart
    /// [`Self::create_xml_save`] returns the dedicated [`AutosarXMLSaver`].
    pub fn create_xml_load(&self) -> Rc<dyn XMLLoader> {
        Rc::new(AutosarXMLLoader::new())
    }

    /// Register this resource's shortName paths into the global
    /// [`AutosarLibraryIndex`] (C++ `indexLibrary`), so later loads can resolve
    /// cross-document references (demand-load).
    pub fn index_library(&self) {
        let contents = self.inner.resource().contents();
        AutosarLibraryIndex::with_global(|idx| idx.index_contents(contents));
    }
}

impl Deref for AutosarResource {
    type Target = XMIResource;
    fn deref(&self) -> &XMIResource {
        &self.inner
    }
}

impl DerefMut for AutosarResource {
    fn deref_mut(&mut self) -> &mut XMIResource {
        &mut self.inner
    }
}

impl Drop for AutosarResource {
    /// Prune this resource's objects from the process-wide ARXML side tables
    /// (C++ `~AutosarXMLResource` → `clearAutosarStoresForObjects`). Those
    /// tables are keyed by object identity, so a resource that does not clean
    /// up after itself leaks an entry per object per load — and, since object
    /// addresses are recycled, a later load may read a previous load's stale
    /// entry. The containment tree is walked depth-first; an unresolved proxy
    /// lives only inside a reference feature (C++ `proxyStore`) and is picked
    /// up when scanning reference values.
    fn drop(&mut self) {
        let mut keys: HashSet<usize> = HashSet::new();
        let mut stack: Vec<ObjectRef> = self.inner.resource().contents().to_vec();
        while let Some(o) = stack.pop() {
            if !keys.insert(store::object_key(&o)) {
                continue;
            }
            let mut proxies: Vec<ObjectRef> = Vec::new();
            {
                let b = o.borrow();
                for c in b.e_contents() {
                    stack.push(c);
                }
                if let Some(dyno) = b.as_any().downcast_ref::<DynamicEObject>() {
                    for f in dyno.structural_features_ref() {
                        if !f.is_reference() {
                            continue;
                        }
                        match dyno.e_get(f.name()) {
                            Some(Val::Object(t)) => proxies.push(t),
                            Some(Val::List(items)) => {
                                proxies.extend(items.iter().filter_map(|v| v.as_object().cloned()))
                            }
                            _ => {}
                        }
                    }
                }
            }
            for t in proxies {
                // Only this resource's unresolved proxies are collected; a
                // resolved target may belong to another resource.
                if t.try_borrow().map(|x| x.e_is_proxy()).unwrap_or(false) {
                    stack.push(t);
                }
            }
        }
        store::clear_for_objects(&keys);
    }
}

/// Present an [`AutosarResource`] through the persistence surface a
/// [`ResourceSet`](emf_common::resource::ResourceSet) can hold.
impl emf_common::resource::ResourceHandle for AutosarResource {
    fn uri(&self) -> &Uri {
        self.inner.resource().uri()
    }
    fn is_loaded(&self) -> bool {
        self.inner.resource().is_loaded()
    }
    fn set_loaded(&mut self, loaded: bool) {
        self.inner.resource_mut().set_loaded(loaded);
    }
    fn contents(&self) -> &[emf_common::value::ObjectRef] {
        self.inner.resource().contents()
    }
    fn set_contents(&mut self, contents: Vec<emf_common::value::ObjectRef>) {
        self.inner.resource_mut().set_contents(contents);
    }
    fn load(&mut self) -> Result<(), String> {
        self.inner.load()
    }
    fn save(&mut self) -> Result<(), String> {
        self.inner.save()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The arxml-flavoured AUTOSAR resource (C++ `AutosarXMLResource`).
///
/// Adds the `createXMLSave` / `createXMLLoad` seam used to inject the arxml
/// serializer/deserializer, and derefs to the base [`AutosarResource`] for
/// everything else.
pub struct AutosarXMLResource {
    base: AutosarResource,
}

impl std::fmt::Debug for AutosarXMLResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutosarXMLResource")
            .field("base", &self.base)
            .finish()
    }
}

impl AutosarXMLResource {
    /// New arxml resource at `uri` resolving its metamodel via `registry`.
    pub fn new(uri: Uri, registry: PackageRegistry) -> Self {
        Self {
            base: AutosarResource::new(uri, registry),
        }
    }

    /// New arxml resource carrying an AUTOSAR `release` descriptor.
    pub fn with_release(
        uri: Uri,
        registry: PackageRegistry,
        release: Option<AutosarReleaseDescriptor>,
    ) -> Self {
        Self {
            base: AutosarResource::with_release(uri, registry, release),
        }
    }

    /// Create the arxml serializer (C++ `AutosarXMLResource::createXMLSave`).
    pub fn create_xml_save(&self) -> Rc<dyn XMLSave> {
        self.base.create_xml_save()
    }

    /// Create the arxml deserializer (C++ `AutosarXMLResource::createXMLLoad`).
    pub fn create_xml_load(&self) -> Rc<dyn XMLLoader> {
        self.base.create_xml_load()
    }

    /// Build the XML helper (C++ `AutosarXMLResource::createXMLHelper`).
    pub fn create_xml_helper(&self) -> XMLHelper {
        self.base.create_xml_helper()
    }
}

impl Deref for AutosarXMLResource {
    type Target = AutosarResource;
    fn deref(&self) -> &AutosarResource {
        &self.base
    }
}

impl DerefMut for AutosarXMLResource {
    fn deref_mut(&mut self) -> &mut AutosarResource {
        &mut self.base
    }
}

/// Present an [`AutosarXMLResource`] through the persistence surface a
/// [`ResourceSet`](emf_common::resource::ResourceSet) can hold.
impl emf_common::resource::ResourceHandle for AutosarXMLResource {
    fn uri(&self) -> &Uri {
        self.base.inner.resource().uri()
    }
    fn is_loaded(&self) -> bool {
        self.base.inner.resource().is_loaded()
    }
    fn set_loaded(&mut self, loaded: bool) {
        self.base.inner.resource_mut().set_loaded(loaded);
    }
    fn contents(&self) -> &[emf_common::value::ObjectRef] {
        self.base.inner.resource().contents()
    }
    fn set_contents(&mut self, contents: Vec<emf_common::value::ObjectRef>) {
        self.base.inner.resource_mut().set_contents(contents);
    }
    fn load(&mut self) -> Result<(), String> {
        self.base.inner.load()
    }
    fn save(&mut self) -> Result<(), String> {
        self.base.inner.save()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
