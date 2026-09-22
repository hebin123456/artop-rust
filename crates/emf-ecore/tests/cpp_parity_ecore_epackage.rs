//! C++ parity suite: emf-ecore EPackage.
//!
//! Ports `EPackageImplTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-ecore/tests/` against Rust `EPackage` +
//! `PackageRegistry` (key-indexed `put`/`get`/`remove` added in the
//! EPackageRegistry parity pass).
use emf_ecore::{make_package_ref, EClass, EClassKind, EPackage, PackageRegistry};

#[test]
fn create_and_get_classifier() {
    let mut p = EPackage::new("MyPkg");
    p.set_ns_uri("http://example.com/MyPkg");
    p.set_ns_prefix("mypkg");
    let c1 = EClass::new("Alpha", EClassKind::Class);
    let c2 = EClass::new("Beta", EClassKind::Class);
    p.add_class(c1);
    p.add_class(c2);

    assert_eq!(p.classes().len(), 2);
    assert!(p.classes().iter().any(|c| c.name() == "Alpha"));
    assert!(p.classes().iter().any(|c| c.name() == "Beta"));
    assert!(!p.classes().iter().any(|c| c.name() == "NotThere"));
}

#[test]
fn registered_in_registry() {
    let mut p = EPackage::new("TestPkg");
    p.set_ns_uri("http://example.com/TestPkg-zzz");
    p.set_ns_prefix("tp");
    let pr = make_package_ref(p);
    let mut reg = PackageRegistry::new();
    reg.put("http://example.com/TestPkg-zzz", pr.clone());
    let got = reg.get("http://example.com/TestPkg-zzz").expect("put/get");
    assert!(std::rc::Rc::ptr_eq(got, &pr));
    reg.remove("http://example.com/TestPkg-zzz");
}

#[test]
fn accessors() {
    let mut p = EPackage::new("X");
    p.set_ns_uri("u");
    p.set_ns_prefix("x");
    assert_eq!(p.name(), "X");
    assert_eq!(p.ns_uri().unwrap().to_string(), "u");
    assert_eq!(p.ns_prefix(), "x");
}
