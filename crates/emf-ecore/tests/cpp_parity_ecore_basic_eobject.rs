//! C++ parity suite: emf-ecore BasicEObject dynamic value storage.
//!
//! Ports `BasicEObjectTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-ecore/tests/` against `DynamicEObject`.
//!
//! Rust maps the C++ *feature-pointer* `eDynamicGet/Set/IsSet/Unset(feature*)`
//! onto the *by-name* `e_get/e_set/e_is_set/e_unset`. Groups not portable at
//! this layer (tracked in PARITY_TRACKER):
//!   - `eContainer` default / after `setEContainer`          -> needs object container field
//!   - `eRegisterInverseList`/`eInverseAdd`/`eInverseRemove` -> needs inverse-list registry
//!   - `eNotificationRequired` + `eSet/eUnset` firing SET/UNSET notifications
//!                                                     -> needs DynamicEObject to be a Notifier
//!   - `eClass` default null                                -> Rust MRequires a class
use emf_ecore::{DynamicEObject, EClass, EClassKind, EStructuralFeature, Val};

fn simple_class() -> EClass {
    let mut cls = EClass::new("Simple", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_feature_id(0);
    name.set_type_name("EString");
    cls.add_feature(name);
    cls
}

// ----- eClass -----

#[test]
fn e_class_returns_set_class() {
    let cls = simple_class();
    let obj = DynamicEObject::new(cls);
    assert_eq!(obj.class().name(), "Simple");
}

// ----- eDynamicGet -----

#[test]
fn e_dynamic_get_unset_returns_null() {
    let obj = DynamicEObject::new(simple_class());
    let v = obj.e_get_by_name("name").expect("known feature");
    assert_eq!(v, Val::Null); // unset -> nothing stored -> null
}

// ----- eDynamicSet / eDynamicGet -----

#[test]
fn e_dynamic_set_then_get() {
    let mut obj = DynamicEObject::new(simple_class());
    assert!(obj.e_set_by_name("name", Val::String("Alice".into())));
    let v = obj.e_get_by_name("name").expect("set value");
    assert!(matches!(&v, Val::String(s) if s == "Alice"));
}

#[test]
fn e_dynamic_set_overwrite() {
    let mut obj = DynamicEObject::new(simple_class());
    obj.e_set_by_name("name", Val::String("first".into()));
    obj.e_set_by_name("name", Val::String("second".into()));
    let v = obj.e_get_by_name("name").expect("set value");
    assert!(matches!(&v, Val::String(s) if s == "second"));
}

#[test]
fn e_dynamic_set_distinct_features() {
    let mut cls = EClass::new("Two", EClassKind::Class);
    let mut a = EStructuralFeature::attribute("a");
    a.set_feature_id(0);
    a.set_type_name("EString");
    let mut b = EStructuralFeature::attribute("b");
    b.set_feature_id(1);
    b.set_type_name("EInt");
    cls.add_feature(a);
    cls.add_feature(b);

    let mut obj = DynamicEObject::new(cls);
    obj.e_set_by_name("a", Val::String("hello".into()));
    obj.e_set_by_name("b", Val::Int(42));

    assert!(matches!(
        obj.e_get_by_name("a").expect("a"),
        Val::String(s) if s == "hello"
    ));
    assert_eq!(obj.e_get_by_name("b").expect("b"), Val::Int(42));
}

// ----- eDynamicIsSet -----

#[test]
fn e_dynamic_is_set_default_false() {
    let obj = DynamicEObject::new(simple_class());
    assert_eq!(obj.e_is_set_by_name("name"), Some(false));
}

#[test]
fn e_dynamic_is_set_after_set_true() {
    let mut obj = DynamicEObject::new(simple_class());
    obj.e_set_by_name("name", Val::String("x".into()));
    assert_eq!(obj.e_is_set_by_name("name"), Some(true));
}

// ----- eDynamicUnset -----

#[test]
fn e_dynamic_unset_clears_value() {
    let mut obj = DynamicEObject::new(simple_class());
    obj.e_set_by_name("name", Val::String("x".into()));
    assert_eq!(obj.e_is_set_by_name("name"), Some(true));
    obj.e_unset_by_name("name");
    assert_eq!(obj.e_is_set_by_name("name"), Some(false));
    assert_eq!(obj.e_get_by_name("name").expect("known"), Val::Null);
}

#[test]
fn e_dynamic_unset_unknown_feature_no_op() {
    // Mirrors the C++ "unset a feature that was never set": no panic.
    let mut obj = DynamicEObject::new(simple_class());
    let unknown = obj.e_unset_by_name("does-not-exist");
    assert!(!unknown);
    assert_eq!(obj.e_is_set_by_name("name"), Some(false));
}
