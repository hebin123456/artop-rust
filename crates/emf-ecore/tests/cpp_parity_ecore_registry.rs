//! C++ parity suite: emf-ecore EPackageRegistry.
//!
//! Ports `EPackageRegistryTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-common/tests/` against the Rust registry API
//! (`register` indexes under `name` / `nsURI` / `nsPrefix`).
use emf_ecore::ecore_package::{ecore_package, ECORE_NS_URI};
use emf_ecore::{make_package_ref, EPackage, PackageRegistry};
use std::rc::Rc;

#[test]
fn put_get_by_ns_uri_ecore() {
    let mut reg = PackageRegistry::new();
    reg.register(ecore_package());
    let p = reg.get(ECORE_NS_URI).expect("ecore registered by nsURI");
    assert_eq!(
        p.borrow().ns_uri().unwrap().to_string(),
        ECORE_NS_URI.to_string()
    );
}

#[test]
fn get_by_name_ecore() {
    let mut reg = PackageRegistry::new();
    reg.register(ecore_package());
    let p = reg.get("ecore").expect("ecore registered by name");
    assert_eq!(p.borrow().name(), "ecore");
}

#[test]
fn get_by_prefix_ecore() {
    let mut reg = PackageRegistry::new();
    reg.register(ecore_package());
    let p = reg.get("ecore").expect("prefix == name");
    assert_eq!(p.borrow().ns_prefix(), "ecore");
}

#[test]
fn contains_key_ecore() {
    let mut reg = PackageRegistry::new();
    reg.register(ecore_package());
    assert!(reg.contains_key("ecore"));
    assert!(reg.contains_key(ECORE_NS_URI));
    assert!(!reg.contains_key("non-existent-nsuri-12345"));
}

#[test]
fn put_custom_package() {
    let mut reg = PackageRegistry::new();
    reg.register(ecore_package());
    let mut p = EPackage::new("test-pkg");
    p.set_ns_uri("http://example.com/test-pkg");
    p.set_ns_prefix("tp");
    let pr = make_package_ref(p);
    reg.put("http://example.com/test-pkg", pr.clone());
    assert!(reg.contains_key("http://example.com/test-pkg"));
    let got = reg.get("http://example.com/test-pkg").expect("put/get");
    assert!(Rc::ptr_eq(got, &pr));
    reg.remove("http://example.com/test-pkg");
}

#[test]
fn keys_include_ecore_ns_uri() {
    let mut reg = PackageRegistry::new();
    reg.register(ecore_package());
    let keys = reg.keys();
    assert!(keys.iter().any(|k| k == ECORE_NS_URI));
    assert!(keys.iter().any(|k| k == "ecore"));
}

#[test]
fn not_found_returns_none() {
    let reg = PackageRegistry::new();
    assert!(reg.get("definitely-not-a-real-key-xyz").is_none());
}
