//! Rust port parity tests for `EValidatorTests.cpp` (C++ `emf-validation`,
//! aligned to Java `org.eclipse.emf.ecore.EValidator`).

use emf_validation::constraint::{Constraint, ConstraintMode, Severity};
use emf_validation::e_validator::EValidator;

// ===== register / get =====
#[test]
fn e_validator_register_and_get() {
    let mut v = EValidator::new();
    v.add_constraint(
        Box::new(|_| true),
        "id1",
        "Name1",
        "msg",
        Severity::Warning,
        ConstraintMode::Batch,
    );
    assert!(v.get_constraint("id1").is_some());
    assert_eq!(v.constraints().len(), 1);
}

// ===== unregister =====
#[test]
fn e_validator_unregister() {
    let mut v = EValidator::new();
    v.add_constraint(
        Box::new(|_| true),
        "id1",
        "Name1",
        "msg",
        Severity::Warning,
        ConstraintMode::Batch,
    );
    assert!(v.unregister_constraint("id1"));
    assert!(v.get_constraint("id1").is_none());
    assert!(!v.unregister_constraint("missing"));
}

// ===== default constraints registered =====
#[test]
fn e_validator_default_constraints_registered() {
    let mut v = EValidator::new();
    v.register_default_constraints();
    assert!(v.get_constraint("emf.validation.default.no_empty_name").is_some());
    assert!(v.get_constraint("emf.validation.default.no_null_required_ref").is_some());
}

// ===== validate =====
#[test]
fn e_validator_validate_returns_violations() {
    let mut v = EValidator::new();
    v.add_constraint(
        Box::new(|_| false),
        "always_fail",
        "Always Fail",
        "violated",
        Severity::Error,
        ConstraintMode::Batch,
    );
    let item = emf_validation::test_util::make_item();
    let diags = v.validate(&*item.borrow());
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].message(), "violated");
}

#[test]
fn e_validator_validate_passing_constraint_yields_none() {
    let mut v = EValidator::new();
    v.add_constraint(
        Box::new(|_| true),
        "always_pass",
        "Always Pass",
        "ok",
        Severity::Info,
        ConstraintMode::Batch,
    );
    let item = emf_validation::test_util::make_item();
    let diags = v.validate(&*item.borrow());
    assert!(diags.is_empty());
}

// Constraint construction is exercised via the registered helpers above.
#[test]
fn constraint_new_holds_parts() {
    let c = Constraint::new(
        Box::new(|_| true),
        "c1",
        "C1",
        "m",
        Severity::Warning,
        ConstraintMode::Live,
    );
    assert_eq!(c.id(), "c1");
    assert_eq!(c.name(), "C1");
    assert_eq!(c.message(), "m");
    assert_eq!(c.severity(), Severity::Warning);
    assert_eq!(c.mode(), ConstraintMode::Live);
}