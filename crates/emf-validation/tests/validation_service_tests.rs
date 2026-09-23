//! Port of C++ `emf-validation/tests/ValidationServiceTests.cpp`.
//!
//! Covers `ValidationService` construction (validator present, include-root on)
//! and the null-input no-op behavior of `validate` / `validateAll`.

use emf_validation::e_validator::EValidator;
use emf_validation::validation_service::ValidationService;

#[test]
fn construct_has_validator() {
    // ValidationService_Construct_HasValidator: getIncludeRoot() == true.
    let svc = ValidationService::new();
    assert!(svc.get_include_root());
}

#[test]
fn validate_null_target_returns_empty() {
    // ValidationService_Validate_NullTarget: an empty validator yields no
    // diagnostics (C++ passes a null target; Rust models it by an empty
    // validator over a regular object).
    let mut svc = ValidationService::new();
    svc.set_validator(EValidator::new());
    let item = emf_validation::test_util::make_item();
    let diags = svc.validate(&*item.borrow());
    assert_eq!(diags.len(), 0);
}

#[test]
fn validate_all_null_root_returns_empty() {
    // ValidationService_ValidateAll_NullRoot: empty validator over a fresh
    // object tree yields no diagnostics.
    let mut svc = ValidationService::new();
    svc.set_validator(EValidator::new());
    let item = emf_validation::test_util::make_item();
    let diags = svc.validate_all(&*item.borrow());
    assert_eq!(diags.len(), 0);
}