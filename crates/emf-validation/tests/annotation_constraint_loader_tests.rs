//! Rust port parity tests for `AnnotationConstraintLoaderTests.cpp`
//! (C++ `emf-validation`, aligned to Java `EObjectValidator` / `EcoreValidator`).
//!
//! Exercises loading OCL / named constraints from `EClass` EAnnotations into an
//! `EValidator`, evaluating them against a `DynamicEObject`.

use emf_ecore::{DynamicEObject, EAnnotation, EClass, EClassKind, EStructuralFeature, Val};
use emf_validation::annotation_constraint_loader::{
    self, CONSTRAINTS_SOURCE, OCL_SOURCE,
};
use emf_validation::e_validator::EValidator;

/// Build a `Foo` EClass carrying a `name` (EString) attribute.
fn make_foo_class() -> EClass {
    let mut cls = EClass::new("Foo", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_type_name("EString");
    cls.add_feature(name);
    cls
}

/// A `Foo` class with an OCL annotation: `name != ''` under `name_nonempty`.
fn make_class_with_ocl_name_constraint() -> EClass {
    let mut cls = make_foo_class();
    let mut ann = EAnnotation::new(OCL_SOURCE);
    ann.set_detail("name_nonempty", "name != ''");
    cls.add_annotation(ann);
    cls
}

// ===== scenario: loadOcl — detail count & id registration =====
#[test]
fn load_ocl_constraints_registers_constraints() {
    let cls = make_class_with_ocl_name_constraint();
    let mut v = EValidator::new();
    let n = annotation_constraint_loader::load_ocl_constraints(&mut v, &cls);
    assert_eq!(n, 1);
    let id = format!("{OCL_SOURCE}#name_nonempty");
    assert!(v.get_constraint(&id).is_some());
}

// ===== scenario: OCL constraint — non-empty name passes =====
#[test]
fn ocl_constraint_name_non_empty_passes() {
    let mut obj = DynamicEObject::new(make_class_with_ocl_name_constraint());
    obj.e_set_by_name("name", Val::string("valid")); // name non-empty
    let mut v = EValidator::new();
    annotation_constraint_loader::load_ocl_constraints(&mut v, obj.class());
    let diags = v.validate(&obj);
    assert_eq!(diags.len(), 0);
}

// ===== scenario: OCL constraint — empty name fails with ERROR =====
#[test]
fn ocl_constraint_name_empty_fails() {
    let mut obj = DynamicEObject::new(make_class_with_ocl_name_constraint());
    obj.e_set_by_name("name", Val::string("")); // name empty -> violation
    let mut v = EValidator::new();
    annotation_constraint_loader::load_ocl_constraints(&mut v, obj.class());
    let diags = v.validate(&obj);
    assert_eq!(diags.len(), 1);
    assert_eq!(
        diags[0].severity(),
        emf_common::diagnostic::Severity::Error
    );
}

// ===== scenario: named constraints — only the declared invariant runs =====
#[test]
fn load_named_constraints_loads_matching_ocl() {
    let mut cls = make_foo_class();
    // OCL annotation provides two invariant expressions.
    let mut ocl = EAnnotation::new(OCL_SOURCE);
    ocl.set_detail("inv1", "name != ''");
    ocl.set_detail("inv2", "name != null");
    cls.add_annotation(ocl);
    // Constraints annotation declares only inv1 should run.
    let mut cons = EAnnotation::new(CONSTRAINTS_SOURCE);
    cons.set_detail("Foo", "inv1");
    cls.add_annotation(cons);

    let mut v = EValidator::new();
    let n = annotation_constraint_loader::load_named_constraints(&mut v, &cls);
    assert_eq!(n, 1); // only inv1
    assert!(v
        .get_constraint(&format!("{CONSTRAINTS_SOURCE}#inv1"))
        .is_some());
    assert!(v
        .get_constraint(&format!("{CONSTRAINTS_SOURCE}#inv2"))
        .is_none());
}

// ===== scenario: no annotation — loads zero =====
#[test]
fn no_annotation_loads_zero_via_load_all() {
    let cls = make_foo_class();
    let mut v = EValidator::new();
    let n = annotation_constraint_loader::load_all(&mut v, &cls);
    assert_eq!(n, 0);
    assert!(v.constraints().is_empty());
}