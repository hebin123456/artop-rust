//! C++ parity suite: emf-ecore ETypedElement metadata.
//!
//! Ports `ETypedElementImplTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-ecore/tests/` against Rust `EStructuralFeature`
//! (the standalone ETypedElement subclass) and `EGenericType`.
//!
//! Portable: default values / isMany semantics / type name (proxy stand-in) /
//! the `EGenericType` lazy-create + eType-sync + union/wildcard group.
//! Tracked gaps: the C++ `eGet`/`eSet`/`eIsSet`/`eUnset` reflection block has
//! no Rust `EStructuralFeature` counterpart (features are accessed by name, not
//! by a feature-pointer-driven reflective `eGet`).
use emf_ecore::structural::FeatureKind;
use emf_ecore::{EGenericType, EStructuralFeature};

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
    // Ports ETypedElement_IsMany: isMany == (upper == -1 || upper > 1).
    let mut e = EStructuralFeature::attribute("e");
    assert!(!e.is_many()); // default upper 1
    e.set_upper_bound(0);
    assert!(!e.is_many()); // 0 is not many (C++ `== -1 || > 1`)
    e.set_upper_bound(2);
    assert!(e.is_many());
    e.set_upper_bound(-1);
    assert!(e.is_many());
}

#[test]
fn bounds_ordered_unique_setters() {
    let mut e = EStructuralFeature::attribute("e");
    e.set_lower_bound(2);
    e.set_upper_bound(-1);
    e.set_ordered(false);
    e.set_unique(false);
    assert_eq!(e.lower_bound(), 2);
    assert_eq!(e.upper_bound(), -1);
    assert!(!e.is_ordered());
    assert!(!e.is_unique());
}

#[test]
fn attribute_vs_reference_kind() {
    let attr = EStructuralFeature::new("x", FeatureKind::Attribute, 0, 1);
    assert!(!attr.is_reference());
    let r = EStructuralFeature::new("y", FeatureKind::Reference, 0, 1);
    assert!(r.is_reference());
    assert!(!r.is_many());
}

// ===== EGenericType: lazy create + eType sync =====

#[test]
fn generic_type_lazy() {
    // Ports ETypedElement_EGenericType_Lazy.
    let mut e = EStructuralFeature::attribute("e");
    // First call creates it (eType null -> eClassifier null).
    assert!(e.generic_type().is_none());
    e.e_generic_type();
    assert!(e.generic_type().is_some());
    assert_eq!(e.generic_type().unwrap().e_classifier(), None);
    // The cached instance is reused.
    let ptr = e.generic_type().unwrap() as *const EGenericType;
    let ptr2 = e.e_generic_type() as *const EGenericType;
    assert_eq!(ptr, ptr2);
}

#[test]
fn set_type_syncs_generic_type() {
    // Ports ETypedElement_SetEType_SyncsGenericType.
    let mut e = EStructuralFeature::attribute("e");
    e.e_generic_type(); // trigger lazy create (classifier null)
    e.set_type_name("X");
    assert_eq!(e.generic_type().unwrap().e_classifier(), Some("X"));
    // Still the same cached generic type.
    let ptr = e.generic_type().unwrap() as *const EGenericType;
    assert_eq!(ptr, e.e_generic_type() as *const EGenericType);
}

#[test]
fn generic_type_follows_e_type() {
    // Ports ETypedElement_GenericTypeFollowsEType.
    let mut e = EStructuralFeature::attribute("e");
    e.set_type_name("dt1");
    assert_eq!(e.e_generic_type().e_classifier(), Some("dt1"));
    e.set_type_name("dt2");
    assert_eq!(e.generic_type().unwrap().e_classifier(), Some("dt2"));
}

// ===== EGenericType: union / wildcard =====

#[test]
fn union_generic_type_arguments() {
    // Ports ETypedElement_Union_GenericTypeArguments: a union is an outer
    // generic type with no classifier but nested type arguments.
    let mut m1 = EGenericType::new();
    m1.set_e_classifier("A");
    let mut m2 = EGenericType::new();
    m2.set_e_classifier("B");
    let mut m3 = EGenericType::new();
    m3.set_e_classifier("C");
    let mut outer = EGenericType::new();
    outer.set_e_type_arguments(vec![m1, m2, m3]);
    assert_eq!(outer.e_type_arguments().len(), 3);
    assert_eq!(outer.e_type_arguments()[0].e_classifier(), Some("A"));
    assert_eq!(outer.e_type_arguments()[1].e_classifier(), Some("B"));
    assert_eq!(outer.e_type_arguments()[2].e_classifier(), Some("C"));
    assert!(outer.is_parameterized());
}

#[test]
fn generic_type_wildcard() {
    // Ports ETypedElement_GenericTypeWildcard: bounds are nested EGenericTypes.
    let mut ub = EGenericType::new();
    ub.set_e_classifier("Upper");
    let mut g = EGenericType::new();
    g.set_e_upper_bound(ub);
    assert_eq!(g.e_upper_bound().unwrap().e_classifier(), Some("Upper"));
    let mut lb = EGenericType::new();
    lb.set_e_classifier("Lower");
    g.set_e_lower_bound(lb);
    assert_eq!(g.e_lower_bound().unwrap().e_classifier(), Some("Lower"));
}

#[test]
fn union_as_nested_generic_type() {
    // Ports ETypedElement_Union_AsNestedGenericType: union container has no
    // classifier itself, only members with classifiers.
    let mut string_t = EGenericType::new();
    string_t.set_e_classifier("EString");
    let mut int_t = EGenericType::new();
    int_t.set_e_classifier("EInt");
    let mut union_gt = EGenericType::new();
    union_gt.set_e_type_arguments(vec![string_t, int_t]);
    assert_eq!(union_gt.e_type_arguments().len(), 2);
    assert!(union_gt.e_type_arguments()[0].e_classifier().is_some());
    assert!(union_gt.e_type_arguments()[1].e_classifier().is_some());
    assert_eq!(union_gt.e_classifier(), None);
}

#[test]
fn generic_type_parameterized_definition() {
    // C++ isEGenericTypeParameterized: eTypeParameter != null ||
    // !eTypeArguments.isEmpty().
    let mut g = EGenericType::new();
    assert!(!g.is_parameterized());
    g.set_e_type_parameter("T");
    assert!(g.is_parameterized());

    let mut e = EStructuralFeature::attribute("e");
    assert!(!e.is_generic_type_parameterized()); // no generic type yet
    e.e_generic_type(); // plain eType only, not parameterized
    assert!(!e.is_generic_type_parameterized());
    let mut args = EGenericType::new();
    args.add_e_type_argument(EGenericType::new());
    e.set_e_generic_type(args);
    assert!(e.is_generic_type_parameterized());
}
