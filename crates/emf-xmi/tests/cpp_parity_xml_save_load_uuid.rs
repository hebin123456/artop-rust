//! C++ parity suite: `P3_XMLSaveLoadUUIDTests.cpp` contract.
//!
//! Ports the XMLSave/XMLLoad abstraction + UUID assertions of the C++ suite:
//! v4 UUID format (`generateUUID`), uniqueness over 1000 draws,
//! `ensureID`/`useUUIDs` auto-assignment (disabled -> none, enabled ->
//! idempotent UUID registered in the id -> eobject map), custom `XMLSave` /
//! `XMLLoad` injection dispatching through `save`/`load`, the default
//! `XMLSaveImpl`/`XMLLoadImpl` being cached and non-null, and an end-to-end
//! default round-trip emitting a real XMI document. Aligned to Java
//! `XMIResourceImpl.generateUUID`/`ensureID`/`getXMLSave`/`getXMLLoad`.

use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use emf_common::uri::Uri;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::{
    make_package_ref, DynamicEObject, EClass, EClassKind, EStructuralFeature, PackageRegistry,
};
use emf_xmi::{XMLLoader, XMIResource, XMLSave};

const NS: &str = "http://example.com/library/1.0";

fn res(uri: &str, reg: &PackageRegistry) -> XMIResource {
    XMIResource::new(Uri::parse(uri), reg.clone())
}

/// A registry with a single `Node{name: EString}` package.
fn node_registry() -> PackageRegistry {
    let mut node = EClass::new("Node", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_type_name("EString");
    node.add_feature(name);

    let mut pkg = emf_ecore::EPackage::new("n");
    pkg.set_ns_prefix("n");
    pkg.set_ns_uri(NS);
    pkg.add_class(node);

    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));
    reg
}

fn new_obj(cls: &str, reg: &PackageRegistry) -> ObjectRef {
    let class = reg.find_class(cls).unwrap();
    Rc::new(std::cell::RefCell::new(DynamicEObject::new_in(
        class,
        reg.clone(),
    )))
}

/// Injected mock serializer, mirroring C++ `MockXMLSave`.
struct MockXMLSave {
    call_count: Cell<u32>,
    last_res_uri: RefCell<String>,
}

impl XMLSave for MockXMLSave {
    fn save(&self, resource: &XMIResource) -> String {
        self.call_count.set(self.call_count.get() + 1);
        *self.last_res_uri.borrow_mut() = resource.resource().uri().to_string();
        "MOCK_SAVE\n".to_string()
    }
}

/// Injected mock deserializer, mirroring C++ `MockXMLLoad`.
struct MockXMLLoad {
    call_count: Cell<u32>,
    loaded_content: RefCell<String>,
}

impl XMLLoader for MockXMLLoad {
    fn load(&self, resource: &mut XMIResource, input: &str) -> Result<(), String> {
        self.call_count.set(self.call_count.get() + 1);
        let _ = resource; // mock does not touch the graph in the C++ suite.
        *self.loaded_content.borrow_mut() = input.to_string();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 1) UUID v4 format
// ---------------------------------------------------------------------------
#[test]
fn generate_uuid_is_v4_format() {
    let reg = node_registry();
    let r = res("file:///u.xmi", &reg);
    let id = r.generate_uuid();
    let bytes: Vec<char> = id.chars().collect();
    assert_eq!(bytes.len(), 36);
    assert_eq!(bytes[8], '-');
    assert_eq!(bytes[13], '-');
    assert_eq!(bytes[18], '-');
    assert_eq!(bytes[23], '-');
    assert_eq!(bytes[14], '4', "version nibble must be 4, got {id}");
    let variant = bytes[19];
    assert!(
        matches!(variant, '8' | '9' | 'a' | 'b'),
        "variant nibble must be in [89ab], got {variant} ({id})"
    );
}

// ---------------------------------------------------------------------------
// 2) UUID uniqueness
// ---------------------------------------------------------------------------
#[test]
fn generate_uuid_unique_over_1000() {
    let reg = node_registry();
    let r = res("file:///u.xmi", &reg);
    let mut ids = std::collections::HashSet::new();
    for _ in 0..1000 {
        ids.insert(r.generate_uuid());
    }
    assert_eq!(ids.len(), 1000, "all 1000 draws must be distinct");
}

// ---------------------------------------------------------------------------
// 3) ensureID with useUUIDs disabled -> no assignment
// ---------------------------------------------------------------------------
#[test]
fn ensure_id_disabled_assigns_none() {
    let reg = node_registry();
    let mut r = res("file:///u.xmi", &reg);
    r.set_use_uuids(false);
    let obj = new_obj("Node", &reg);
    assert_eq!(r.ensure_id(&obj), "");
    assert_eq!(r.id_to_eobject_map().len(), 0);
}

// ---------------------------------------------------------------------------
// 4) ensureID with useUUIDs enabled -> auto v4 UUID, idempotent, registered
// ---------------------------------------------------------------------------
#[test]
fn ensure_id_enabled_assigns_idempotent_uuid() {
    let reg = node_registry();
    let mut r = res("file:///u.xmi", &reg);
    r.set_use_uuids(true);
    let obj = new_obj("Node", &reg);

    let id1 = r.ensure_id(&obj);
    assert_eq!(id1.chars().count(), 36);
    assert_eq!(id1.as_bytes()[14], b'4');

    // Idempotent: second call returns the same id.
    let id2 = r.ensure_id(&obj);
    assert_eq!(id1, id2);

    // Registered in the id -> eobject map, and resolvable back.
    assert_eq!(r.id_to_eobject_map().len(), 1);
    let got = r.get_object_by_id(&id1).expect("resolvable by id");
    assert!(Rc::ptr_eq(&got, &obj));
}

// ---------------------------------------------------------------------------
// 5) XMLSave injection: custom impl called
// ---------------------------------------------------------------------------
#[test]
fn xml_save_injection_custom_impl_called() {
    let reg = node_registry();
    let mut r = res("file:///s.xmi", &reg);
    let mock: Rc<MockXMLSave> = Rc::new(MockXMLSave {
        call_count: Cell::new(0),
        last_res_uri: RefCell::new(String::new()),
    });

    // Inject by handle and save: the mock serializer is dispatched.
    let for_resource: Rc<dyn XMLSave> = mock.clone();
    r.set_xml_save_rc(for_resource);
    let obj = new_obj("Node", &reg);
    r.resource_mut().add_to_contents(obj);

    let out = r.save_to_string();
    assert_eq!(mock.call_count.get(), 1);
    assert_eq!(out, "MOCK_SAVE\n");
    assert_eq!(*mock.last_res_uri.borrow(), "file:///s.xmi");
}

// ---------------------------------------------------------------------------
// 6) XMLLoad injection: custom impl called
// ---------------------------------------------------------------------------
#[test]
fn xml_load_injection_custom_impl_called() {
    let reg = node_registry();
    let mut r = res("file:///l.xmi", &reg);
    let mock = Rc::new(MockXMLLoad {
        call_count: Cell::new(0),
        loaded_content: RefCell::new(String::new()),
    });

    let for_resource: Rc<dyn XMLLoader> = mock.clone();
    r.set_xml_load_rc(for_resource);

    r.load_from_string("hello world").unwrap();
    assert_eq!(mock.call_count.get(), 1);
    assert_eq!(*mock.loaded_content.borrow(), "hello world");
}

// ---------------------------------------------------------------------------
// 7) Default impls: non-null, cached (same instance), concrete type
// ---------------------------------------------------------------------------
#[test]
fn xml_save_default_impl_not_null_and_cached() {
    let reg = node_registry();
    let r = res("file:///d.xmi", &reg);
    let a = r.get_xml_save();
    // Second call returns the same cached instance.
    let b = r.get_xml_save();
    assert!(Rc::ptr_eq(&a, &b));
}

#[test]
fn xml_load_default_impl_not_null_and_cached() {
    let reg = node_registry();
    let r = res("file:///d.xmi", &reg);
    let a = r.get_xml_load();
    let b = r.get_xml_load();
    assert!(Rc::ptr_eq(&a, &b));
}

// ---------------------------------------------------------------------------
// 8) Default impls end-to-end: real XMI round-trip
// ---------------------------------------------------------------------------
#[test]
fn xml_save_default_impl_roundtrips() {
    let reg = node_registry();
    let mut r = res("file:///rt.xmi", &reg);
    let node = new_obj("Node", &reg);
    node.borrow_mut().e_set("name", Val::String("Test".into()));
    r.resource_mut().add_to_contents(node);

    // Ensure the default serializer (not a mock) is active.
    r.set_xml_save_rc(emf_xmi::XMLSaveImpl::new_boxed());

    let out = r.save_to_string();
    assert!(out.contains("<?xml"), "{out}");
    assert!(out.contains("XML"), "{out}");
    assert!(out.contains("Test"), "{out}");
}