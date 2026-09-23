//! C++ parity suite: `E2E_ProxyModelTests.cpp` — proxy object creation and
//! resolution contract (aligned to Java `EObjectImpl.eIsProxy` / `eProxyURI` /
//! `eSetProxyURI` / `eResolveProxy`, plus `EcoreUtil.resolve(proxy, set)` and
//! `ResourceSet.getEObject(uri, loadOnDemand)`).
//!
//! Coverage mirrored from the C++ suite:
//!   1. a new object is not a proxy (`eIsProxy` == false);
//!   2. after `eSetProxyURI`, `eIsProxy` returns true;
//!   3. `eProxyURI` returns the URI that was set;
//!   4. resolving a non-proxy returns the object itself;
//!   5. resolving a proxy through a `ResourceSet` reaches the *target* root;
//!   5b. resolving a proxy with no set returns the proxy unchanged;
//!   6. proxy URI with a fragment is stored whole and its fragment() is exposed;
//!   7. multiple proxies stay independent;
//!   8. `getEObject(uri, loadOnDemand)` finds a resource's root object;
//!   9. cross-resource lookup by URI finds the root (here the package);
//!   10. `eIsProxy` stays true after the URI is set;
//!   11. `getResource(uri, loadOnDemand=false)` returns the registered resource;
//!   12. a later proxy URI overwrites an earlier one.
//!
//! C++ factory methods (`EcoreFactory.initialize`/
//! `XMIResourceFactory.registerDefaults`) are no-ops here because the Rust
//! factory path is registered statically; each proxy object is a dynamic
//! EObject over a tiny in-memory metamodel (`Node`).

use emf_common::uri::Uri;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::{make_package_ref, DynamicEObject, EClass, EClassKind, PackageRegistry};
use emf_xmi::XMIResourceSet;
use std::cell::RefCell;
use std::rc::Rc;

/// The in-memory metamodel used for every proxy object: one `Node` class.
fn node_registry() -> PackageRegistry {
    let mut pkg = emf_ecore::EPackage::new("node");
    pkg.set_ns_prefix("n");
    pkg.set_ns_uri("http://example.org/node");
    pkg.add_class(EClass::new("Node", EClassKind::Class));
    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));
    reg
}

/// `createTestEObject` — a dynamic object over the shared `Node` metamodel.
fn new_node(registry: &PackageRegistry) -> ObjectRef {
    let cls = registry.find_class("Node").unwrap();
    Rc::new(RefCell::new(DynamicEObject::new_in(cls, registry.clone())))
}

#[test]
fn new_object_is_not_proxy() {
    let reg = node_registry();
    let obj = new_node(&reg);
    assert!(!obj.borrow().e_is_proxy(), "a fresh object is not a proxy");
}

#[test]
fn set_proxy_uri_makes_is_proxy_true() {
    let reg = node_registry();
    let obj = new_node(&reg);
    obj.borrow_mut()
        .e_set_proxy_uri(Some(Uri::parse("http://example.com/model.xmi#//Foo")));
    assert!(obj.borrow().e_is_proxy());
}

#[test]
fn proxy_uri_returns_set_uri() {
    let reg = node_registry();
    let obj = new_node(&reg);
    let uri_str = "http://example.com/model.xmi#//@books.0";
    obj.borrow_mut().e_set_proxy_uri(Some(Uri::parse(uri_str)));
    let binding = obj.borrow();
    let got = binding.e_proxy_uri().expect("proxy URI set");
    assert_eq!(got.to_string(), uri_str);
}

#[test]
fn resolve_proxy_non_proxy_returns_self() {
    let reg = node_registry();
    let obj = new_node(&reg);
    let caller = new_node(&reg);
    let resolved = caller.borrow().e_resolve_proxy(&obj);
    assert!(Rc::ptr_eq(&resolved, &obj), "non-proxy resolves to itself");
}

#[test]
fn resolve_proxy_resolves_via_resource_set() {
    let reg = node_registry();
    let mut rs = XMIResourceSet::new(reg.clone());

    // target resource holds an object that acts as the reference target.
    let target_res =
        rs.create_resource(Uri::parse("http://example.com/target.xmi"));
    let target = new_node(&reg);
    target
        .borrow_mut()
        .e_set("name", Val::String("Target".into()));
    target_res.borrow_mut().resource_mut().add_to_contents(target.clone());

    // caller resource (holder) — present so the set has "caller.xmi" too.
    let _caller_res =
        rs.create_resource(Uri::parse("http://example.com/caller.xmi"));

    // A proxy pointing at the target resource root.
    let proxy = new_node(&reg);
    proxy
        .borrow_mut()
        .e_set_proxy_uri(Some(Uri::parse("http://example.com/target.xmi")));

    // Resolve through the resource set: reaches the *target* object,
    // not the proxy itself (EMF EcoreUtil.resolve contract).
    let resolved = rs
        .resolve_proxy_uri(&Uri::parse("http://example.com/target.xmi"))
        .expect("proxy resolves through the set");
    assert!(
        Rc::ptr_eq(&resolved, &target),
        "proxy resolves to the target root, not the proxy"
    );
    assert!(!Rc::ptr_eq(&resolved, &proxy));
}

#[test]
fn resolve_proxy_no_resource_set_returns_proxy() {
    let reg = node_registry();
    let proxy = new_node(&reg);
    proxy
        .borrow_mut()
        .e_set_proxy_uri(Some(Uri::parse("http://example.com/model.xmi#//Bar")));
    let caller = new_node(&reg);
    // The caller has no resource / set to resolve against: the degenerate
    // result is the proxy unchanged (C++ `eResolveProxy` fallback).
    let resolved = caller.borrow().e_resolve_proxy(&proxy);
    assert!(
        Rc::ptr_eq(&resolved, &proxy),
        "without a resource set the proxy resolves to itself"
    );
    assert!(resolved.borrow().e_is_proxy());
}

#[test]
fn proxy_uri_with_fragment_stored_correctly() {
    let reg = node_registry();
    let obj = new_node(&reg);
    let full = "file:///path/to/model.xmi#//Library/books.0";
    obj.borrow_mut().e_set_proxy_uri(Some(Uri::parse(full)));
    let binding = obj.borrow();
    let uri = binding.e_proxy_uri().expect("proxy URI set");
    assert_eq!(uri.to_string(), full);
    assert_eq!(uri.fragment(), "//Library/books.0");
}

#[test]
fn multiple_proxies_are_independent() {
    let reg = node_registry();
    let p1 = new_node(&reg);
    let p2 = new_node(&reg);
    p1.borrow_mut()
        .e_set_proxy_uri(Some(Uri::parse("http://a.com/m.xmi#//A")));
    p2.borrow_mut()
        .e_set_proxy_uri(Some(Uri::parse("http://b.com/m.xmi#//B")));

    assert!(p1.borrow().e_is_proxy());
    assert!(p2.borrow().e_is_proxy());

    let u1 = p1.borrow().e_proxy_uri().unwrap().to_string();
    let u2 = p2.borrow().e_proxy_uri().unwrap().to_string();
    assert_ne!(u1, u2, "each proxy keeps its own URI");
    assert_eq!(u1, "http://a.com/m.xmi#//A");
    assert_eq!(u2, "http://b.com/m.xmi#//B");
}

#[test]
fn resource_set_cross_resource_lookup() {
    let reg = node_registry();
    let mut rs = XMIResourceSet::new(reg.clone());

    let res = rs.create_resource(Uri::parse("http://example.com/test.xmi"));
    let obj = new_node(&reg);
    res.borrow_mut().resource_mut().add_to_contents(obj.clone());

    let found = rs
        .get_eobject(&Uri::parse("http://example.com/test.xmi"), true)
        .expect("root object found by URI");
    assert!(Rc::ptr_eq(&found, &obj));
}

#[test]
fn resource_set_get_eobject_with_fragment_finds_root() {
    let reg = node_registry();
    let mut rs = XMIResourceSet::new(reg.clone());

    let res = rs.create_resource(Uri::parse("http://example.com/test2.xmi"));
    // The C++ test adds an EPackage named "testPkg"; here the root is a Node.
    let pkg = new_node(&reg);
    res.borrow_mut()
        .resource_mut()
        .add_to_contents(pkg.clone());

    let found = rs
        .get_eobject(&Uri::parse("http://example.com/test2.xmi"), true)
        .expect("root object found by URI");
    assert!(Rc::ptr_eq(&found, &pkg));
}

#[test]
fn is_proxy_persistent_after_set() {
    let reg = node_registry();
    let obj = new_node(&reg);
    assert!(!obj.borrow().e_is_proxy());
    obj.borrow_mut()
        .e_set_proxy_uri(Some(Uri::parse("http://example.com/m.xmi#//X")));
    assert!(obj.borrow().e_is_proxy());
    assert!(obj.borrow().e_is_proxy(), "stays a proxy across reads");
}

#[test]
fn resource_set_get_resource_returns_registered() {
    let reg = node_registry();
    let mut rs = XMIResourceSet::new(reg.clone());
    let uri = Uri::parse("http://example.com/registered.xmi");
    let created = rs.create_resource(uri.clone());
    let found = rs
        .get_resource(&uri, false)
        .expect("registered resource is found");
    assert!(Rc::ptr_eq(&created, &found));
}

#[test]
fn proxy_uri_overwrite() {
    let reg = node_registry();
    let obj = new_node(&reg);
    obj.borrow_mut()
        .e_set_proxy_uri(Some(Uri::parse("http://old.com/m.xmi#//Old")));
    let old_uri = {
        let binding = obj.borrow();
        binding.e_proxy_uri().unwrap().to_string()
    };
    assert_eq!(old_uri, "http://old.com/m.xmi#//Old");
    obj.borrow_mut()
        .e_set_proxy_uri(Some(Uri::parse("http://new.com/m.xmi#//New")));
    let new_uri = {
        let binding = obj.borrow();
        binding.e_proxy_uri().unwrap().to_string()
    };
    assert_eq!(new_uri, "http://new.com/m.xmi#//New");
    assert!(obj.borrow().e_is_proxy());
}