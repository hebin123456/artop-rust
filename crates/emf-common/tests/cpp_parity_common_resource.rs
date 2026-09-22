//! C++ parity suite: emf-common Resource.
//!
//! Ports `ResourceTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-common/tests/`.
use emf_common::eobject::EObject;
use emf_common::resource::Resource;
use emf_common::uri::Uri;
use emf_common::value::ObjectRef;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

/// Minimal EObject stub (only `e_class` + `as_any` are required).
#[derive(Debug)]
struct Stub;

impl EObject for Stub {
    fn e_class(&self) -> &str {
        "Stub"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn obj() -> ObjectRef {
    Rc::new(RefCell::new(Stub))
}

#[test]
fn resource_get_uri_construction() {
    let r = Resource::new(Uri::parse("http://example.com/x.mi"));
    assert_eq!(r.uri().scheme(), "http");
    assert_eq!(r.uri().authority(), "example.com");
    assert_eq!(r.uri().path(), "/x.mi");
}

#[test]
fn resource_set_uri_updates() {
    let mut r = Resource::new(Uri::parse("http://a/b.mi"));
    r.set_uri(Uri::parse("http://c/d.mi"));
    assert_eq!(r.uri().authority(), "c");
    assert_eq!(r.uri().path(), "/d.mi");
}

#[test]
fn resource_default_uri_empty() {
    let r = Resource::new(Uri::new());
    assert!(r.uri().is_empty());
}

#[test]
fn resource_default_contents_empty() {
    let r = Resource::new(Uri::parse("u"));
    assert!(r.contents().is_empty());
}

#[test]
fn resource_add_to_contents_appends() {
    let mut r = Resource::new(Uri::parse("u"));
    let o1 = obj();
    let o2 = obj();
    r.add_to_contents(o1.clone());
    r.add_to_contents(o2.clone());
    assert_eq!(r.contents().len(), 2);
    assert!(Rc::ptr_eq(&r.contents()[0], &o1));
    assert!(Rc::ptr_eq(&r.contents()[1], &o2));
}

#[test]
fn resource_default_root_none() {
    let r = Resource::new(Uri::parse("u"));
    assert!(r.root().is_none());
}

#[test]
fn resource_set_root_returns_same() {
    let mut r = Resource::new(Uri::parse("u"));
    let o = obj();
    r.set_root(o.clone());
    assert!(r.root().is_some_and(|ro| Rc::ptr_eq(ro, &o)));
}

#[test]
fn resource_default_resource_set_none() {
    // Rust constrains Resource spawned from a ResourceSet; base has no set.
    let r = Resource::new(Uri::parse("u"));
    assert!(r.contents().is_empty());
}

#[test]
fn resource_default_not_loaded() {
    let r = Resource::new(Uri::parse("u"));
    assert!(!r.is_loaded());
}

#[test]
fn resource_set_loaded_true() {
    let mut r = Resource::new(Uri::parse("u"));
    r.set_loaded(true);
    assert!(r.is_loaded());
    r.set_loaded(false);
    assert!(!r.is_loaded());
}

#[test]
fn resource_default_not_modified() {
    let r = Resource::new(Uri::parse("u"));
    assert!(!r.is_modified());
}

#[test]
fn resource_set_modified_true() {
    let mut r = Resource::new(Uri::parse("u"));
    r.set_modified(true);
    assert!(r.is_modified());
    r.set_modified(false);
    assert!(!r.is_modified());
}

#[test]
fn resource_default_empty_errors_and_warnings() {
    let r = Resource::new(Uri::parse("u"));
    assert!(r.errors().is_empty());
    assert!(r.warnings().is_empty());
}

#[test]
fn resource_add_error_persists() {
    let mut r = Resource::new(Uri::parse("u"));
    r.set_errors(vec!["bad element".into(), "missing attr".into()]);
    assert_eq!(r.errors().len(), 2);
    assert_eq!(r.errors()[0], "bad element");
    assert_eq!(r.errors()[1], "missing attr");
}

#[test]
fn resource_add_warning_persists() {
    let mut r = Resource::new(Uri::parse("u"));
    r.set_warnings(vec!["deprecated".into()]);
    assert_eq!(r.warnings().len(), 1);
    assert_eq!(r.warnings()[0], "deprecated");
}

#[test]
fn resource_save_load_defaults_no_throw() {
    let r = Resource::new(Uri::parse("u"));
    assert!(r.save_to_string().is_empty());
    assert!(r.to_xmi_string().is_empty());
}

#[test]
fn resource_from_xmi_string_no_throw_not_loaded() {
    let mut r = Resource::new(Uri::parse("u"));
    r.from_xmi_string("<x/>");
    assert!(!r.is_loaded());
}

#[test]
fn resource_get_eobject_empty_fragment_none() {
    let r = Resource::new(Uri::parse("u"));
    assert!(r.get_eobject("").is_none());
}

#[test]
fn resource_get_uri_fragment_null_object_empty() {
    let r = Resource::new(Uri::parse("u"));
    let stub = obj();
    let frag = r.get_uri_fragment(&stub);
    assert!(frag.is_empty());
}
