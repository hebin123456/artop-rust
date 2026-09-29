//! AUTOSAR resource factories (port of C++ `AutosarResourceFactory` /
//! `AutosarXMLResourceFactory`, aligned to Java
//! `org.artop.aal.common.resource.impl.AutosarResourceFactoryImpl` /
//! `AutosarXMLResourceFactoryImpl`).
//!
//! A factory's job (mirroring the C++ behaviour):
//!   1. create a resource instance (`create_resource`);
//!   2. inject the default load/save options (`init_resource` /
//!      `init_default_options`), including the release's
//!      `xsi:schemaLocation`;
//!   3. maintain the schema-location catalog (`namespace -> xsd`).
//!
//! Rust has no virtual inheritance, so the C++ split between the abstract
//! `AutosarResourceFactory` and its XML flavour is collapsed into one concrete
//! [`AutosarResourceFactory`] producing [`AutosarXMLResource`]s by default;
//! callers override the produced resource with [`set_resource_creator`].
//!
//! [`set_resource_creator`]: AutosarResourceFactory::set_resource_creator

use std::collections::HashMap;
use std::rc::Rc;

use emf_common::resource::ResourceHandle;
use emf_common::uri::Uri;
use emf_ecore::PackageRegistry;
use emf_xmi::XMIResource;

use crate::autosar_resource::{AutosarResource, AutosarXMLResource};
use crate::release_descriptor::AutosarReleaseDescriptor;

/// A resource constructor: `(uri, registry) -> resource handle` (the Rust
/// counterpart of the C++ `AutosarResourceCreator` lambda).
pub type AutosarResourceCreator = Rc<dyn Fn(&Uri, &PackageRegistry) -> Box<dyn ResourceHandle>>;

/// Creates AUTOSAR resources carrying a release descriptor (C++
/// `AutosarResourceFactory` + `AutosarXMLResourceFactory`).
pub struct AutosarResourceFactory {
    release: Option<AutosarReleaseDescriptor>,
    registry: PackageRegistry,
    catalog: HashMap<String, String>,
    creator: Option<AutosarResourceCreator>,
}

impl AutosarResourceFactory {
    /// A factory with no release and an empty metamodel registry.
    pub fn new(release: Option<AutosarReleaseDescriptor>) -> Self {
        Self {
            release,
            registry: PackageRegistry::default(),
            catalog: HashMap::new(),
            creator: None,
        }
    }

    /// A factory whose produced resources resolve their metamodel via
    /// `registry`.
    pub fn with_registry(
        release: Option<AutosarReleaseDescriptor>,
        registry: PackageRegistry,
    ) -> Self {
        Self {
            release,
            registry,
            catalog: HashMap::new(),
            creator: None,
        }
    }

    /// The associated release descriptor (C++ `getAutosarRelease`).
    pub fn autosar_release(&self) -> Option<&AutosarReleaseDescriptor> {
        self.release.as_ref()
    }

    /// Replace the release descriptor (C++ `setAutosarRelease`).
    pub fn set_autosar_release(&mut self, release: Option<AutosarReleaseDescriptor>) {
        self.release = release;
    }

    /// Register a custom resource constructor (C++ `setResourceCreator`); the
    /// produced resource replaces the default [`AutosarXMLResource`].
    pub fn set_resource_creator(&mut self, creator: AutosarResourceCreator) {
        self.creator = Some(creator);
    }

    /// The schema-location catalog (`namespace -> xsd`).
    pub fn schema_location_catalog(&self) -> &HashMap<String, String> {
        &self.catalog
    }

    /// Add an entry to the schema-location catalog (C++
    /// `addSchemaLocation`).
    pub fn add_schema_location(&mut self, ns: impl Into<String>, schema: impl Into<String>) {
        self.catalog.insert(ns.into(), schema.into());
    }

    /// Build the default schema-location catalog for the release (C++
    /// `AutosarXMLResourceFactory::createSchemaLocationCatalog`): the AUTOSAR
    /// base namespace plus the two XML namespaces.
    pub fn create_schema_location_catalog(&self) -> HashMap<String, String> {
        let mut cat = self.catalog.clone();
        if let Some(release) = &self.release {
            cat.insert(
                release.base_namespace().to_string(),
                release.schema_location(),
            );
        }
        cat.insert(
            "http://www.w3.org/XML/1998/namespace".to_string(),
            "xml.xsd".to_string(),
        );
        cat.insert(
            "http://www.w3.org/2001/XMLSchema-instance".to_string(),
            "XML.xsd".to_string(),
        );
        cat
    }

    /// Inject default options into a resource (C++ `initDefaultOptions`,
    /// aligned to Java `initDefaultOptions`).
    pub fn init_default_options(&self, res: &mut XMIResource) {
        res.options_mut().encoding = "UTF-8".to_string();
        res.options_mut().xmi_version = "2.0".to_string();
    }

    /// Initialise a resource: default options plus, when a release is known,
    /// its `xsi:schemaLocation` (C++ `initResource`).
    pub fn init_resource(&self, res: &mut XMIResource) {
        self.init_default_options(res);
        if let Some(release) = &self.release {
            res.set_xsi_schema_location(release.schema_location());
        }
    }

    /// Create a resource for `uri` (C++ `createResource`). Uses the registered
    /// creator when present, otherwise a default [`AutosarXMLResource`] with
    /// the release and schema location applied.
    pub fn create_resource(&self, uri: Uri) -> Box<dyn ResourceHandle> {
        if let Some(creator) = &self.creator {
            return creator(&uri, &self.registry);
        }
        let mut res =
            AutosarXMLResource::with_release(uri, self.registry.clone(), self.release.clone());
        if let Some(release) = &self.release {
            res.set_schema_location(release.schema_location());
        }
        Box::new(res)
    }

    /// Register the base namespace's `xsi:schemaLocation` on an existing AUTOSAR
    /// resource (convenience over `init_resource` for the AUTOSAR resource type).
    pub fn init_autosar_resource(&self, res: &mut AutosarResource) {
        self.init_resource(res);
        if let Some(release) = &self.release {
            res.set_schema_location(release.schema_location());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version_data::AutosarMetaModelVersionData;

    fn release_448() -> AutosarReleaseDescriptor {
        AutosarReleaseDescriptor::new(
            "org.artop.aal.autosar448".to_string(),
            AutosarMetaModelVersionData::new(4, 4, 8),
        )
    }

    #[test]
    fn creates_autosar_xml_resource_with_release() {
        let factory = AutosarResourceFactory::new(Some(release_448()));
        let res = factory.create_resource(Uri::parse("file:///tmp/t.arxml"));
        assert_eq!(res.uri().to_string(), "file:///tmp/t.arxml");
        let xml = res
            .as_any()
            .downcast_ref::<AutosarXMLResource>()
            .expect("default resource is AutosarXMLResource");
        assert_eq!(
            xml.schema_location(),
            "http://autosar.org/schema/r4.0 AUTOSAR_4-4-8.xsd"
        );
        assert!(xml.autosar_release().is_some());
    }

    #[test]
    fn custom_creator_replaces_default() {
        let reg = PackageRegistry::default();
        let mut factory = AutosarResourceFactory::new(Some(release_448()));
        factory.set_resource_creator(Rc::new(|uri: &Uri, reg: &PackageRegistry| {
            Box::new(AutosarResource::new(uri.clone(), reg.clone()))
        }));
        let res = factory.create_resource(Uri::parse("file:///tmp/t.arxml"));
        assert!(res.as_any().downcast_ref::<AutosarResource>().is_some());
        let _ = reg;
    }

    #[test]
    fn init_resource_injects_schema_location() {
        let factory = AutosarResourceFactory::new(Some(release_448()));
        let mut res = XMIResource::new(
            Uri::parse("file:///tmp/t.arxml"),
            PackageRegistry::default(),
        );
        factory.init_resource(&mut res);
        assert!(res.get_xsi_schema_location().contains("AUTOSAR_4-4-8.xsd"));
    }

    #[test]
    fn schema_catalog_has_base_namespace() {
        let factory = AutosarResourceFactory::new(Some(release_448()));
        let cat = factory.create_schema_location_catalog();
        assert!(cat.contains_key("http://autosar.org/schema/r4.0"));
        assert_eq!(
            cat.get("http://autosar.org/schema/r4.0").unwrap(),
            "http://autosar.org/schema/r4.0 AUTOSAR_4-4-8.xsd"
        );
    }
}
