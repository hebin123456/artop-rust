//! `LiveValidator` — a `ValidationLiveAdapter` bridging an [`EValidator`] to
//! live (constraint-on-change) validation (port of C++ `emf-validation`
//! `LiveValidator` / `ValidationLiveAdapter`, aligned to Java
//! `org.eclipse.emf.validation.service.ValidationLiveAdapter`).
//!
//! The adapter owns a reference to a shared [`EValidator`], carries an enable
//! flag, and can be `attach`ed to a root object, with zero or more listeners
//! that are notified whenever a live validation runs.
//!
//! # Notification trade-off vs C++ `EContentAdapter`
//!
//! In C++/Java the adapter extends `EContentAdapter`: `attach(root)` recursively
//! installs a notifier on every object in the containment subtree, and the
//! framework's `eSet`/`add`/`remove` fire `notifyChanged`, which automatically
//! re-validates the mutated object and dispatches to listeners.
//!
//! Rust's [`DynamicEObject`](emf_ecore::DynamicEObject) emits *no* change
//! events, so there is no callback path from an `e_set` on a subtree object back
//! to the adapter (and re-reading the very object being mutated from inside its
//! own `RefCell::borrow_mut` would deadlock). We therefore keep the adapter's
//! attachment as *state* (the attached root + registered listeners) and drive
//! live validation explicitly through [`ValidationLiveAdapter::validate_now`],
//! which runs the `LIVE` constraint set and dispatches the diagnostics to all
//! listeners. Callers re-run `validate_now` after mutating the model, matching
//! the existing `artop-validation` convention for the same reason.

use crate::constraint::ConstraintMode;
use crate::e_validator::EValidator;
use emf_common::diagnostic::Diagnostic;
use emf_common::eobject::EObject;
use emf_common::value::ObjectRef;

/// A live-validation listener callback: `(target, diagnostics)` once per run.
pub type LiveValidationListener = dyn FnMut(&dyn EObject, &[Diagnostic]);

/// Live-validation adapter over an [`EValidator`].
pub struct ValidationLiveAdapter {
    validator: EValidator,
    enabled: bool,
    /// The root object this adapter is attached to, if any.
    attached_root: Option<ObjectRef>,
    /// Registered live-validation listeners.
    listeners: Vec<Box<LiveValidationListener>>,
}

impl ValidationLiveAdapter {
    /// New adapter driven by the given validator (owns it).
    pub fn new(validator: EValidator) -> Self {
        Self {
            validator,
            enabled: true,
            attached_root: None,
            listeners: Vec::new(),
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

    /// The root this adapter is attached to, if any.
    pub fn attached_root(&self) -> Option<ObjectRef> {
        self.attached_root.clone()
    }

    /// Register a live-validation listener. It is invoked with `(mutated
    /// target, diagnostics)` whenever a live validation runs on a target.
    pub fn add_listener(
        &mut self,
        listener: impl FnMut(&dyn EObject, &[Diagnostic]) + 'static,
    ) {
        self.listeners.push(Box::new(listener));
    }

    /// Attach the adapter to `root`. Replaces any previous attachment.
    ///
    /// In C++ this recursively installs the `EContentAdapter` notifier on the
    /// whole containment subtree. Rust `DynamicEObject` emits no change events,
    /// so here we simply record `root`; live re-validation is driven explicitly
    /// by [`Self::validate_now`] (see the module doctrings for the trade-off).
    pub fn attach(&mut self, root: ObjectRef) {
        if self.attached_root.is_some() {
            self.detach();
        }
        self.attached_root = Some(root);
    }

    /// Detach from the previously attached root and clear the listener set.
    pub fn detach(&mut self) {
        self.attached_root = None;
        self.listeners.clear();
    }

    /// Run the live constraint set over `target` now and dispatch the result to
    /// all registered listeners. A disabled adapter returns no diagnostics and
    /// does not notify listeners.
    pub fn validate_now(&mut self, target: &dyn EObject) -> Vec<Diagnostic> {
        if !self.enabled {
            return Vec::new();
        }
        let diags = self.validator.validate_mode(target, Some(ConstraintMode::Live));
        for l in &mut self.listeners {
            (l)(target, &diags);
        }
        diags
    }

    /// The underlying validator (mutable).
    pub fn validator(&mut self) -> &mut EValidator {
        &mut self.validator
    }
}