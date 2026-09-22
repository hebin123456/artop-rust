//! C++ parity suite: emf-ecore ETypedElement metadata.
//!
//! Ports `ETypedElementImplTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-ecore/tests/` against Rust `EStructuralFeature`.
//!
//! Portable: default values / isMany semantics / type name (proxy stand-in).
//! Tracked gaps: the C++ `eGet`/`eSet`/`eIsSet`/`eUnset` reflection block, the
//! `setLowerBound`/`setUpperBound`/`setOrdered`/`setUnique` setters, and the
//! `EGenericType` (union/wildcard/lazy-sync) machinery have no Rust
//! `EStructuralFeature` counterpart yet.
use emf_ecore::structural::FeatureKind;
use emf_ecore::EStructuralFeature;

#[test]
fn defaults() {
    let a = EStructuralFeature::attribute("a");
    assert_eq!(a.lower_bound(), 0);
    assert_eq!(a.upper_bound(), 1);
    assert!(a.is_ordered());
    assert!(a.is_unique());
    assert!(!a.is_many());
    assert_eq!(a.type_name(), None); // eType null
}

#[test]
fn set_type_name() {
    let mut a = EStructuralFeature::attribute("a");
    a.set_type_name("MyType");
    assert_eq!(a.type_name(), Some("MyType"));
}

#[test]
fn is_many_semantics() {
    let single = EStructuralFeature::attribute("s"); // upper 1
    assert!(!single.is_many());
    let many = EStructuralFeature::reference_many("m"); // upper -1
    assert!(many.is_many());
    assert!(many.is_reference());
}

#[test]
fn attribute_vs_reference_kind() {
    let attr = EStructuralFeature::new("x", FeatureKind::Attribute, 0, 1);
    assert!(!attr.is_reference());
    let r = EStructuralFeature::new("y", FeatureKind::Reference, 0, 1);
    assert!(r.is_reference());
    assert!(!r.is_many());
}
