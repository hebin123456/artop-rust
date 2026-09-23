//! `ValidationService` — aggregate an [`EValidator`] and run it over a single
//! object or a whole containment tree (port of C++ `emf-validation`
//! `ValidationService`, aligned to Java
//! `org.eclipse.emf.validation.service.ModelValidationService`).
//!
//! Constraint-mode handling mirrors the C++ batching contract: `validate`
//! runs only `ConstraintMode::Batch` constraints; `include_live_constraints`
//! additionally admits `Live` ones.

use crate::constraint::ConstraintMode;
use crate::e_validator::EValidator;
use emf_common::diagnostic::Diagnostic;
use emf_common::eobject::EObject;
use emf_common::value::ObjectRef;

/// Aggregated validation service over a single [`EValidator`].
///
/// The validator is owned by the service (C++ holds it via `unique_ptr`); a
/// service is cheap and owns its own validator by default.
pub struct ValidationService {
    validator: EValidator,
    include_root: bool,
    include_live: bool,
}

impl Default for ValidationService {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationService {
    /// A new service with an empty batch validator.
    pub fn new() -> Self {
        Self {
            validator: EValidator::new(),
            include_root: true,
            include_live: false,
        }
    }

    /// The underlying validator (mutable).
    pub fn validator(&mut self) -> &mut EValidator {
        &mut self.validator
    }

    /// The underlying validator (shared).
    pub fn validator_ref(&self) -> &EValidator {
        &self.validator
    }

    /// Replace the underlying validator.
    pub fn set_validator(&mut self, validator: EValidator) {
        self.validator = validator;
    }

    /// Whether `validate_all` also validates the root itself (default `true`).
    pub fn get_include_root(&self) -> bool {
        self.include_root
    }

    /// Set whether `validate_all` validates the root itself.
    pub fn set_include_root(&mut self, b: bool) {
        self.include_root = b;
    }

    /// Whether batch validation also runs `Live` constraints (default `false`).
    pub fn get_include_live_constraints(&self) -> bool {
        self.include_live
    }

    /// Set whether batch validation also runs `Live` constraints.
    pub fn set_include_live_constraints(&mut self, b: bool) {
        self.include_live = b;
    }

    /// The effective constraint mode for a batch run, honoring the live-include
    /// flag. A `None` mode means "all constraints".
    fn batch_mode(&self) -> Option<ConstraintMode> {
        if self.include_live {
            None
        } else {
            Some(ConstraintMode::Batch)
        }
    }

    /// Validate a single object (batch: `Batch` constraints only).
    pub fn validate(&self, target: &dyn EObject) -> Vec<Diagnostic> {
        self.validator.validate_mode(target, self.batch_mode())
    }

    /// Validate an object and its full containment tree.
    ///
    /// `include_root == false` validates the children only. The walk follows
    /// `e_contents()` depth-first.
    pub fn validate_all(&self, root: &dyn EObject) -> Vec<Diagnostic> {
        let mode = self.batch_mode();
        let mut out = Vec::new();
        if self.include_root {
            collect(root, &self.validator, mode, &mut out);
        } else {
            for child in root.e_contents() {
                collect(&*child.borrow(), &self.validator, mode, &mut out);
            }
        }
        out
    }
}

/// Depth-first walk validating every object reachable via containment.
fn collect(
    obj: &dyn EObject,
    validator: &EValidator,
    mode: Option<ConstraintMode>,
    out: &mut Vec<Diagnostic>,
) {
    out.extend(validator.validate_mode(obj, mode));
    for child in obj.e_contents() {
        collect(&*child.borrow(), validator, mode, out);
    }
}

/// Convenience helper to reach `ObjectRef` values from containment when needed.
#[allow(dead_code)]
fn successors(o: &dyn EObject) -> Vec<ObjectRef> {
    o.e_contents()
}