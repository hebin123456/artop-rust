//! Integration tests for XSD schema incorporation (`import` / `include`),
//! ported from the C++ `P5_XSDSchemaIncorporateTests.cpp` (empty in C++) and
//! aligned to Java `XSDSchemaImpl.incorporate` / `XSDResource`.
//!
//! Behaviour is asserted through the public [`XSDResource`] / [`XSDSchemaRegistry`]
//! API plus the observable metamodel state (`referencing_directives`,
//! `incorporated_schema`, target-namespace fallback).

use std::rc::Rc;

use emf_xsd::resource::{XSDResource, XSDSchemaRegistry};
use emf_xsd::xsd_metamodel::{
    XSDImport, XSDInclude, XSDSchemaCompositor, XSDSchemaRef, XsdDirectiveKind,
};

/// A schema in namespace `urn:common` with one global type/definition.
const COMMON_NS: &str = r#"<?xml version="1.0"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
           targetNamespace="urn:common"
           elementFormDefault="qualified">
  <xs:complexType name="CommonType">
    <xs:sequence>
      <xs:element name="part" type="xs:string"/>
    </xs:sequence>
  </xs:complexType>
</xs:schema>"#;

/// Same components but *without* a `targetNamespace` (chameleon schema).
const COMMON_NO_NS: &str = r#"<?xml version="1.0"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
           elementFormDefault="qualified">
  <xs:complexType name="CommonType">
    <xs:sequence>
      <xs:element name="part" type="xs:string"/>
    </xs:sequence>
  </xs:complexType>
</xs:schema>"#;

/// The owning schema, in namespace `urn:main`.
const MAIN_NS: &str = r#"<?xml version="1.0"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
           targetNamespace="urn:main"
           elementFormDefault="qualified">
  <xs:import namespace="urn:common" schemaLocation="common.xsd"/>
  <xs:include schemaLocation="common.xsd"/>
  <xs:element name="root" type="xs:string"/>
</xs:schema>"#;

fn parse(xml: &str) -> XSDSchemaRef {
    XSDResource::parse_schema_from_string(xml).expect("schema parses")
}

/// Fresh thread-global registry state for a test (the harness runs each test on
/// its own thread, and the registry is thread-local, but be explicit).
fn reset() {
    XSDSchemaRegistry::with_global(|registry| registry.clear());
}

fn register(location: &str, schema: XSDSchemaRef) {
    XSDSchemaRegistry::with_global(|registry| registry.register_schema(location, schema));
}

#[test]
fn registry_register_find_clear() {
    reset();
    let schema = parse(COMMON_NS);

    XSDSchemaRegistry::with_global(|registry| {
        // Empty locations are ignored.
        registry.register_schema("", Rc::clone(&schema));
        assert!(registry.find_by_location("").is_none());

        registry.register_schema("common.xsd", Rc::clone(&schema));
        assert!(Rc::ptr_eq(
            &registry.find_by_location("common.xsd").unwrap(),
            &schema
        ));

        registry.clear();
        assert!(registry.find_by_location("common.xsd").is_none());
    });
}

#[test]
fn resolve_schema_prefers_cache_over_loader() {
    reset();
    let cached = parse(COMMON_NS);
    register("common.xsd", Rc::clone(&cached));

    // A loader that would produce a *different* schema if consulted.
    XSDSchemaRegistry::with_global(|registry| {
        registry.set_loader(Box::new(|_loc| {
            Some(Rc::new(std::cell::RefCell::new(emf_xsd::XSDSchema::new())))
        }));
    });

    let resolved = XSDResource::resolve_schema("common.xsd").expect("cached");
    assert!(
        Rc::ptr_eq(&resolved, &cached),
        "cache must win over the loader"
    );
}

#[test]
fn resolve_schema_uses_loader_and_handles_misses() {
    reset();
    let loaded = parse(COMMON_NS);
    XSDSchemaRegistry::with_global(|registry| {
        registry.set_loader(Box::new(move |loc| {
            if loc == "common.xsd" {
                Some(Rc::clone(&loaded))
            } else {
                None
            }
        }));
    });

    assert!(XSDResource::resolve_schema("common.xsd").is_some());
    assert!(XSDResource::resolve_schema("missing.xsd").is_none());
    // Empty location never resolves.
    assert!(XSDResource::resolve_schema("").is_none());
}

#[test]
fn import_with_schema_location_incorporates() {
    reset();
    let common = parse(COMMON_NS);
    register("common.xsd", Rc::clone(&common));
    let owning = parse(MAIN_NS);

    let import = XSDImport {
        namespace: Some("urn:common".into()),
        schema_location: Some("common.xsd".into()),
    };
    let mut compositor = XSDSchemaCompositor::from_import(&import);
    compositor.schema = Some(Rc::clone(&owning));

    let incorporated = XSDResource::resolve_and_incorporate(&mut compositor).expect("resolved");

    // The resolved schema is the cached one, and both forward edges are set.
    assert!(Rc::ptr_eq(&incorporated, &common));
    assert!(Rc::ptr_eq(
        compositor.resolved_schema.as_ref().unwrap(),
        &common
    ));
    assert!(Rc::ptr_eq(
        compositor.incorporated_schema.as_ref().unwrap(),
        &common
    ));

    // Reverse edge recorded on the incorporated schema.
    let directives = &common.borrow().referencing_directives;
    assert_eq!(directives.len(), 1);
    assert_eq!(directives[0].kind, XsdDirectiveKind::Import);
    assert_eq!(directives[0].namespace.as_deref(), Some("urn:common"));
    assert_eq!(directives[0].schema_location.as_deref(), Some("common.xsd"));
    assert_eq!(XsdDirectiveKind::Import.to_string(), "import");

    // Same schema resolved & incorporated → it is its own original version.
    assert!(compositor.original_version().is_none());
}

#[test]
fn import_without_schema_location_is_noop() {
    reset();
    let import = XSDImport {
        namespace: Some("urn:common".into()),
        schema_location: None,
    };
    let mut compositor = XSDSchemaCompositor::from_import(&import);

    let result = XSDResource::resolve_and_incorporate(&mut compositor);
    assert!(result.is_none());
    assert!(compositor.resolved_schema.is_none());
    assert!(compositor.incorporated_schema.is_none());
}

#[test]
fn include_without_target_namespace_inherits_owning_namespace() {
    reset();
    let chameleon = parse(COMMON_NO_NS);
    assert!(chameleon.borrow().target_namespace.is_none());
    register("common.xsd", Rc::clone(&chameleon));
    let owning = parse(MAIN_NS);

    let include = XSDInclude {
        schema_location: "common.xsd".into(),
    };
    let mut compositor = XSDSchemaCompositor::from_include(&include);
    compositor.schema = Some(Rc::clone(&owning));

    let incorporated = XSDResource::resolve_and_incorporate(&mut compositor).expect("resolved");
    assert!(Rc::ptr_eq(&incorporated, &chameleon));

    // Target-namespace fallback: chameleon schema adopts the owning namespace.
    assert_eq!(
        chameleon.borrow().target_namespace.as_deref(),
        Some("urn:main")
    );
    assert_eq!(
        chameleon.borrow().referencing_directives[0].kind,
        XsdDirectiveKind::Include
    );
}

#[test]
fn include_keeps_its_own_target_namespace() {
    reset();
    let common = parse(COMMON_NS);
    register("common.xsd", Rc::clone(&common));
    let owning = parse(MAIN_NS);

    let mut compositor = XSDSchemaCompositor::from_include(&XSDInclude {
        schema_location: "common.xsd".into(),
    });
    compositor.schema = Some(Rc::clone(&owning));

    XSDResource::resolve_and_incorporate(&mut compositor).expect("resolved");
    assert_eq!(
        common.borrow().target_namespace.as_deref(),
        Some("urn:common"),
        "an explicit targetNamespace must not be overwritten by the fallback"
    );
}

#[test]
fn original_version_is_resolved_schema_when_it_differs() {
    // When the resolved schema is not the one finally incorporated (e.g. a
    // cloned version), getOriginalVersion points at the resolved schema.
    let resolved = parse(COMMON_NS);
    let incorporated = parse(COMMON_NS);
    let mut compositor = XSDSchemaCompositor::from_include(&XSDInclude {
        schema_location: "common.xsd".into(),
    });
    compositor.resolved_schema = Some(Rc::clone(&resolved));
    compositor.incorporated_schema = Some(Rc::clone(&incorporated));

    let original = compositor.original_version().expect("differs");
    assert!(Rc::ptr_eq(original, &resolved));
}

#[test]
fn parse_schema_from_string_reports_errors() {
    reset();
    assert!(XSDResource::parse_schema_from_string(COMMON_NS).is_ok());
    let err = XSDResource::parse_schema_from_string("<not-a-schema/>").unwrap_err();
    assert!(
        err.contains("no <schema> element"),
        "unexpected error: {err}"
    );
}
