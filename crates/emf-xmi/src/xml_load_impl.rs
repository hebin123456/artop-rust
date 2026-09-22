//! `XMLLoad` / `XMLLoadImpl` — the XML deserialization entry point.
//!
//! Port of C++ `emf::xmi::XMLLoad` / `XMLLoadImpl` (aligned to Java
//! `org.eclipse.emf.ecore.xmi.XMLLoad` / `impl.XMLLoadImpl`).
//!
//! `XMLLoad` is the abstract `load(resource, input, options)` surface;
//! `XMLLoadImpl` is the default implementation. In the C++ port it delegates to
//! a free `loadInto()`; in Rust the equivalent lives in [`super::loader`]
//! (`load_from_str`) and in [`super::XMIResource::load_from_string`], which
//! feeds the resource's own [`emf_common::resource::PackageRegistry`]-driven
//! reflection. This module provides the trait surface plus a thin adapter so
//! callers can use the standard "XMLLoad" vocabulary.
//!
//! Generic EMF only: no domain-metamodel knowledge (see `docs/PROGRESS.md`).

use emf_common::value::ObjectRef;

use super::xmi_resource::XMIResource;

/// Options observed by an [`XMLLoad`]; a `()` payload for now, extensible when
/// XMI behavior toggles land.
pub type XMLLoadOptions = ();

/// The `(resource, input, options)` triple a load operates on.
pub struct LoadRequest<'a> {
    /// The target resource.
    pub resource: &'a mut XMIResource,
    /// The XMI/XML text to parse.
    pub input: &'a str,
    /// Load options.
    pub options: XMLLoadOptions,
}

/// XML deserialization interface (C++ `XMLLoad`; Java `XMLLoad`).
pub trait XMLLoad {
    /// Parse `request.input` into `request.resource`, honoring `request.options`.
    fn load(&mut self, request: LoadRequest<'_>) -> Result<(), String>;
}

/// Default [`XMLLoad`] implementation (C++ `XMLLoadImpl`).
///
/// Delegates to the resource's own registry-driven loader, mirroring the way
/// the C++ `XMLLoadImpl::load` forwards to `loadInto()`.
#[derive(Debug, Default)]
pub struct XMLLoadImpl;

impl XMLLoadImpl {
    /// New default implementation.
    pub fn new() -> Self {
        Self
    }
}

impl XMLLoad for XMLLoadImpl {
    fn load(&mut self, request: LoadRequest<'_>) -> Result<(), String> {
        request.resource.load_from_string(request.input)
    }
}

/// Validate that a string parses as XMI without committing it to a resource.
/// Convenience that mirrors C++'s parser-level entry points.
pub fn parse_only(
    registry: &emf_ecore::PackageRegistry,
    src: &str,
) -> Result<Vec<ObjectRef>, String> {
    super::loader::load_from_str(src, registry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_common::uri::Uri;
    use emf_ecore::{make_package_ref, EClass, EClassKind, EStructuralFeature, PackageRegistry};

    fn reg() -> PackageRegistry {
        let mut pkg = emf_ecore::EPackage::new("d");
        pkg.set_ns_prefix("d");
        let mut item = EClass::new("Item", EClassKind::Class);
        let mut name = EStructuralFeature::attribute("name");
        name.set_type_name("EString");
        item.add_feature(name);
        pkg.add_class(item);
        let mut r = PackageRegistry::new();
        r.register(make_package_ref(pkg));
        r
    }

    #[test]
    fn xmi_load_impl_roundtrips_into_resource() {
        let registry = reg();
        let xmi = r#"<d:Item name="widget"/>"#;
        let mut res = XMIResource::new(Uri::parse("file:///x.xmi"), registry);
        let mut loader = XMLLoadImpl::new();
        loader
            .load(LoadRequest {
                resource: &mut res,
                input: xmi,
                options: (),
            })
            .unwrap();
        assert!(res.resource().is_loaded());
        let contents = res.resource().contents();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].borrow().e_class(), "Item");
        assert_eq!(
            contents[0].borrow().e_get("name"),
            Some(emf_common::value::Val::String("widget".into()))
        );
    }

    #[test]
    fn load_bad_input_reports_error() {
        let registry = reg();
        let mut res = XMIResource::new(Uri::parse("file:///y.xmi"), registry);
        let mut loader = XMLLoadImpl::new();
        let err = loader
            .load(LoadRequest {
                resource: &mut res,
                input: "<d:NoSuchClass/>",
                options: (),
            })
            .unwrap_err();
        assert!(!err.is_empty());
        assert!(!res.resource().is_loaded());
    }

    #[test]
    fn parse_only_validates_without_mutation() {
        let registry = reg();
        let roots = parse_only(&registry, "<d:Item name=\"a\"/>").unwrap();
        assert_eq!(roots.len(), 1);
    }
}
