//! C++ parity suite: emf-ecore DynamicEObject value storage.
//!
//! Ports `DynamicEObjectImplTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-ecore/tests/` against Rust `DynamicEObject`
//! (feature-access by *name*; C++ uses feature pointers).
//!
//! Tracked gaps (need object container + containment wiring):
//!   - `eContents` collection, `eContainer`/`eContainmentFeature` auto-set on
//!     containment add/overwrite, multi-valued reference lists.
use emf_ecore::{DynamicEObject, EClass, EClassKind, EStructuralFeature, Val};

fn node_class() -> EClass {
    let mut cls = EClass::new("Node", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_feature_id(0);
    let mut value = EStructuralFeature::attribute("value");
    value.set_feature_id(1);
    cls.add_feature(name);
    cls.add_feature(value);
    cls
}

#[test]
fn e_class_returns_constructor_class() {
    let obj = DynamicEObject::new(node_class());
    assert_eq!(obj.class().name(), "Node");
}

#[test]
fn e_set_e_get_string_attribute() {
    let mut obj = DynamicEObject::new(node_class());
    obj.e_set_by_name("name", Val::String("root".into()));
    let v = obj.e_get_by_name("name").expect("set");
    assert!(matches!(&v, Val::String(s) if s == "root"));
}

#[test]
fn e_set_e_get_int_attribute() {
    let mut obj = DynamicEObject::new(node_class());
    obj.e_set_by_name("value", Val::Int(42));
    let v = obj.e_get_by_name("value").expect("set");
    assert_eq!(v, Val::Int(42));
}

#[test]
fn e_get_unset_attribute_null() {
    let obj = DynamicEObject::new(node_class());
    assert_eq!(obj.e_get_by_name("name"), Some(Val::Null));
}

#[test]
fn e_is_set_default_false_after_set_true() {
    let mut obj = DynamicEObject::new(node_class());
    assert_eq!(obj.e_is_set_by_name("name"), Some(false));
    obj.e_set_by_name("name", Val::String("x".into()));
    assert_eq!(obj.e_is_set_by_name("name"), Some(true));
}

#[test]
fn e_unset_clears_value() {
    let mut obj = DynamicEObject::new(node_class());
    obj.e_set_by_name("name", Val::String("x".into()));
    assert_eq!(obj.e_is_set_by_name("name"), Some(true));
    obj.e_unset_by_name("name");
    assert_eq!(obj.e_is_set_by_name("name"), Some(false));
}

#[test]
fn e_set_overwrite_attribute() {
    let mut obj = DynamicEObject::new(node_class());
    obj.e_set_by_name("name", Val::String("first".into()));
    obj.e_set_by_name("name", Val::String("second".into()));
    let v = obj.e_get_by_name("name").expect("set");
    assert!(matches!(&v, Val::String(s) if s == "second"));
}

#[test]
fn unknown_feature_returns_none() {
    let mut obj = DynamicEObject::new(node_class());
    assert!(!obj.e_set_by_name("nope", Val::Int(1)));
    assert_eq!(obj.e_get_by_name("nope"), None);
    assert_eq!(obj.e_is_set_by_name("nope"), None);
}