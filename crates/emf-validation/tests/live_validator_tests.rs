//! Port of C++ `emf-validation/tests/LiveValidatorTests.cpp`.
//!
//! Covers `ValidationLiveAdapter` construction (enabled by default), the
//! `setEnabled`/`isEnabled` toggle and the null-input no-op of `validateNow`.

use emf_validation::e_validator::EValidator;
use emf_validation::live_validator::ValidationLiveAdapter;

#[test]
fn construct() {
    // LiveValidator_Construct: a fresh adapter is enabled by default.
    let live = ValidationLiveAdapter::new(EValidator::new());
    assert!(live.is_enabled());
}

#[test]
fn set_enabled() {
    // LiveValidator_SetEnabled: setEnabled(false) flips isEnabled().
    let mut live = ValidationLiveAdapter::new(EValidator::new());
    live.set_enabled(false);
    assert!(!live.is_enabled());
}

#[test]
fn validate_now_null_target_returns_empty() {
    // LiveValidator_ValidateNow_NullTarget: an empty adapter over a regular
    // object yields no diagnostics (C++ passes a null target; Rust models it
    // by an empty validator over a normal object).
    let mut live = ValidationLiveAdapter::new(EValidator::new());
    let item = emf_validation::test_util::make_item();
    let diags = live.validate_now(&*item.borrow());
    assert_eq!(diags.len(), 0);
}