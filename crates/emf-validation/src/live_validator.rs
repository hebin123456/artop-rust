//! `LiveValidator` — a `ValidationLiveAdapter` bridging an [`EValidator`] to
//! live (constraint-on-change) validation (port of C++ `emf-validation`
//! `LiveValidator` / `ValidationLiveAdapter`, aligned to Java
//! `org.eclipse.emf.validation.service.ValidationLiveAdapter`).
//!
//! The adapter owns a reference to a shared [`EValidator`] and carries an
//! enable flag. A `None`-target `validate_now` yields no diagnostics (C++).

use crate::constraint::ConstraintMode;
use crate::e_validator::EValidator;
use emf_common::diagnostic::Diagnostic;
use emf_common::eobject::EObject;

/// Live-validation adapter over an [`EValidator`].
pub struct ValidationLiveAdapter {
    validator: EValidator,
    enabled: bool,
}

impl ValidationLiveAdapter {
    /// New adapter driven by the given validator (owns it).
    pub fn new(validator: EValidator) -> Self {
        Self {
            validator,
            enabled: true,
        }
    }

    /// Whether live validation is enabled (default `true`).
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable live validation.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Run the live constraint set over `target` now. A `None` target is a
    /// no-op (C++ returns an empty list for `nullptr`).
    pub fn validate_now(&self, target: &dyn EObject) -> Vec<Diagnostic> {
        if !self.enabled {
            return Vec::new();
        }
        self.validator.validate_mode(target, Some(ConstraintMode::Live))
    }

    /// The underlying validator (mutable).
    pub fn validator(&mut self) -> &mut EValidator {
        &mut self.validator
    }
}