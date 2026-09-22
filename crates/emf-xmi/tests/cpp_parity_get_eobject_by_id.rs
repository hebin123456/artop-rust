//! C++ parity suite: `P3_5_GetEObjectByIDHrefTests.cpp` contract.
//!
//! Ports the ID / href / position-path navigation assertions of the C++ suite:
//! `XMIResource.setID`/`getID`/`getEObjectByID` (bidirectional, overwrite,
//! unknown-id), `xmi:id` auto-registration on load, `getEObject(fragment)` in
//! the `"?<id>"` / `"Name"` / `"//Name"` forms, `resolvePositionPath` for
//! `@feat.index` position paths and classifier names, and the `getIDToEObjectMap`
//! accessor. Aligned to Java `XMIResourceImpl` / `XMLHelperImpl.getID`/`getHREF`.
//!
//! The C++ suite navigates an `ecore:EPackage` metamodel; in Rust the
//! metamodel loader yields plain `EPackage` structs, so these object-graph
//! behaviors are exercised against `DynamicEObject` documents hosted in an
//! [`XMIResource`] (the runtime counterpart of the same ID/href contract).

use emf_common::uri::Uri;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::{
    make_package_ref, DynamicEObject, EClass, EClassKind, EStructuralFeature, PackageRegistry,
};
use emf_xmi::XMIResource;
use std::cell::RefCell;
use std::rc::Rc;

const NS: &str = "http://example.com/library/1.0";

fn res(uri: &str, reg: &PackageRegistry) -> XMIResource {
    XMIResource::new(Uri::parse(uri), reg.clone())
}

/// A registry with `Library{name:toString, books: containment Book}` and
/// `Book{title:toString}` (the C++ library ecore).
fn library_registry() -> PackageRegistry {
    let mut book = EClass::new("Book", EClassKind::Class);
    let mut title = EStructuralFeature::attribute("title");
    title.set_type_name("EString");
    book.add_feature(title);

    let mut library = EClass::new("Library", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_type_name("EString");
    let mut books = EStructuralFeature::reference_many("books");
    books.set_containment(true);
    books.set_type_name("Book");
    library.add_feature(name);
    library.add_feature(books);

    let mut pkg = emf_ecore::EPackage::new("library");
    pkg.set_ns_prefix("library");
    pkg.set_ns_uri(NS);
    pkg.add_class(library);
    pkg.add_class(book);

    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));
    reg
}

fn new_obj(cls: &str, reg: &PackageRegistry) -> ObjectRef {
    let class = reg.find_class(cls).unwrap();
    Rc::new(RefCell::new(DynamicEObject::new_in(class, reg.clone())))
}

/// Build `Library` with `name` and two contained `Book`s `b0`,`b1`.
#[allow(clippy::type_complexity)]
fn build_library(reg: &PackageRegistry) -> (ObjectRef, ObjectRef, ObjectRef) {
    let b0 = new_obj("Book", reg);
    b0.borrow_mut().e_set("title", Val::String("B0".into()));
    let b1 = new_obj("Book", reg);
    b1.borrow_mut().e_set("title", Val::String("B1".into()));
    let lib = new_obj("Library", reg);
    lib.borrow_mut().e_set("name", Val::String("Lib".into()));
    lib.borrow_mut().e_set(
        "books",
        Val::List(vec![Val::Object(b0.clone()), Val::Object(b1.clone())]),
    );
    (lib, b0, b1)
}

// ---------------------------------------------------------------------------
// 1) setID / getID round-robin
// ---------------------------------------------------------------------------
#[test]
fn set_id_get_id_roundtrip() {
    let reg = library_registry();
    let mut res = res("file:///r.xmi", &reg);
    let pkg = new_obj("Book", &res.registry());
    res.set_id(&pkg, "pkg1");
    assert_eq!(res.get_id(&pkg), "pkg1");
    let got = res.get_object_by_id("pkg1").expect("id resolves");
    assert!(Rc::ptr_eq(&got, &pkg));
}

// ---------------------------------------------------------------------------
// 2) setID overwrite: new id replaces old; old no longer resolves
// ---------------------------------------------------------------------------
#[test]
fn set_id_overwrites_old() {
    let reg = library_registry();
    let mut res = res("file:///r.xmi", &reg);
    let pkg = new_obj("Book", &res.registry());
    res.set_id(&pkg, "old");
    assert_eq!(res.get_id(&pkg), "old");
    res.set_id(&pkg, "new");
    assert_eq!(res.get_id(&pkg), "new");
    assert!(res.get_object_by_id("old").is_none());
    let got = res.get_object_by_id("new").unwrap();
    assert!(Rc::ptr_eq(&got, &pkg));
}

// ---------------------------------------------------------------------------
// 3) get_object_by_id unknown id -> None
// ---------------------------------------------------------------------------
#[test]
fn unknown_id_returns_none() {
    let reg = library_registry();
    let res = res("file:///r.xmi", &reg);
    assert!(res.get_object_by_id("does-not-exist").is_none());
}

// ---------------------------------------------------------------------------
// 4) get_id unregistered object -> empty string
// ---------------------------------------------------------------------------
#[test]
fn get_id_unregistered_object_empty() {
    let reg = library_registry();
    let res = res("file:///r.xmi", &reg);
    let pkg = new_obj("Book", &res.registry());
    assert_eq!(res.get_id(&pkg), "");
}

// ---------------------------------------------------------------------------
// 5) xmi:id auto-registers on load
// ---------------------------------------------------------------------------
#[test]
fn xmi_id_registered_on_load() {
    let reg = library_registry();
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<xmi:XMI xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:library="http://example.com/library/1.0">
  <library:Book xmi:id="_idA" title="A"/>
  <library:Book xmi:id="_idB" title="B"/>
</xmi:XMI>"#;
    let mut res = res("file:///r.xmi", &reg);
    res.load_from_string(xml).unwrap();
    let a = res.get_object_by_id("_idA").expect("_idA resolves");
    let b = res.get_object_by_id("_idB").expect("_idB resolves");
    assert_eq!(a.borrow().e_get("title"), Some(Val::String("A".into())));
    assert_eq!(b.borrow().e_get("title"), Some(Val::String("B".into())));
    assert_eq!(res.get_id(&a), "_idA");
    assert_eq!(res.get_id(&b), "_idB");
}

// ---------------------------------------------------------------------------
// 6) get_eobject("?id") -> by xmi:id
// ---------------------------------------------------------------------------
#[test]
fn get_eobject_question_mark_id() {
    let reg = library_registry();
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<xmi:XMI xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:library="http://example.com/library/1.0">
  <library:Book xmi:id="_idA" title="A"/>
</xmi:XMI>"#;
    let mut res = res("file:///r.xmi", &reg);
    res.load_from_string(xml).unwrap();
    let a = res.get_eobject("?_idA").expect("?_idA resolves");
    assert_eq!(a.borrow().e_get("title"), Some(Val::String("A".into())));
}

// ---------------------------------------------------------------------------
// 7) get_eobject("Name") -> by class name
// ---------------------------------------------------------------------------
#[test]
fn get_eobject_by_class_name() {
    let reg = library_registry();
    let mut res = res("file:///r.xmi", &reg);
    let (lib, _b0, _b1) = build_library(&reg);
    res.resource_mut().set_contents(vec![lib]);
    // both Library (by name) and inner Book are discoverable.
    assert!(res.get_eobject("Library").is_some());
    assert!(res.get_eobject("Book").is_some());
}

// ---------------------------------------------------------------------------
// 8) get_eobject("//Name") -> strip leading slash then by name
// ---------------------------------------------------------------------------
#[test]
fn get_eobject_double_slash_class_name() {
    let reg = library_registry();
    let mut res = res("file:///r.xmi", &reg);
    let (lib, _b0, _b1) = build_library(&reg);
    res.resource_mut().set_contents(vec![lib]);
    assert!(res.get_eobject("//Library").is_some());
}

// ---------------------------------------------------------------------------
// 9) get_eobject("") -> None
// ---------------------------------------------------------------------------
#[test]
fn get_eobject_empty_fragment_none() {
    let reg = library_registry();
    let res = res("file:///r.xmi", &reg);
    assert!(res.get_eobject("").is_none());
}

// ---------------------------------------------------------------------------
// 10) get_eobject unknown name -> None
// ---------------------------------------------------------------------------
#[test]
fn get_eobject_unknown_name_none() {
    let reg = library_registry();
    let mut res = res("file:///r.xmi", &reg);
    let (lib, _b0, _b1) = build_library(&reg);
    res.resource_mut().set_contents(vec![lib]);
    assert!(res.get_eobject("NoSuchClass").is_none());
}

// ---------------------------------------------------------------------------
// 11) resolve_position_path("@books.0") -> first contained book; @books.1 -> b1
// ---------------------------------------------------------------------------
#[test]
fn resolve_position_path_first_child() {
    let reg = library_registry();
    let mut res = res("file:///r.xmi", &reg);
    let (lib, b0, b1) = build_library(&reg);
    res.resource_mut().set_contents(vec![lib]);
    let got0 = res.resolve_position_path("@books.0").expect("@books.0");
    assert!(Rc::ptr_eq(&got0, &b0));
    let got1 = res.resolve_position_path("@books.1").expect("@books.1");
    assert!(Rc::ptr_eq(&got1, &b1));
}

// ---------------------------------------------------------------------------
// 12) resolve_position_path("Library") -> by class name
// ---------------------------------------------------------------------------
#[test]
fn resolve_position_path_classifier_name() {
    let reg = library_registry();
    let mut res = res("file:///r.xmi", &reg);
    let (lib, _b0, _b1) = build_library(&reg);
    res.resource_mut().set_contents(vec![lib]);
    assert!(res.resolve_position_path("Library").is_some());
    assert!(res.resolve_position_path("Book").is_some());
}

// ---------------------------------------------------------------------------
// 13) id_to_eobject map contains all registered ids
// ---------------------------------------------------------------------------
#[test]
fn id_map_contains_all_registered() {
    let reg = library_registry();
    let mut res = res("file:///r.xmi", &reg);
    let a = new_obj("Book", &res.registry());
    let b = new_obj("Book", &res.registry());
    res.set_id(&a, "a");
    res.set_id(&b, "b");
    let map = res.id_to_eobject_map();
    assert_eq!(map.len(), 2);
    assert!(map.contains_key("a"));
    assert!(map.contains_key("b"));
}