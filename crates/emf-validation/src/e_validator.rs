//! `EValidator` — execute all registered constraints against a single `EObject`
//! and return diagnostics (port of C++ `emf-validation` `EValidator`, aligned
//! to Java `org.eclipse.emf.ecore.EValidator`).
//!
//! Provides constraint registration/unregistration, a per-`EPackage` registry
//! (for `Diagnostician` dispatch) and default built-in constraints.

use crate::constraint::{Constraint, ConstraintMode, IConstraintListener, Severity};
use emf_common::eobject::EObject;
use emf_common::value::Val;
use emf_ecore::DynamicEObject;

/// Re-export severity so callers see a single validation-centric name source.
pub use crate::constraint::Severity as ValidationSeverity;

/// Legacy process-wide diagnostic source identifier. Kept as a public constant
/// for backwards compatibility, but the built-in constraints now stamp each
/// diagnostic with the *constraint name* (C++ `Constraint::getName()`), so this
/// constant is no longer used by `validate_mode`.
pub const DIAGNOSTIC_SOURCE: &str = "org.eclipse.emf.validation";

/// Default constraint evaluator: a feature literally named `"name"` must not
/// hold an empty string (C++ `noEmptyNameEval`). Objects without a `name`
/// feature (or where it isn't set) pass.
fn no_empty_name_eval(target: &dyn EObject) -> bool {
    let name = target
        .e_get("name")
        .and_then(|v| v.as_str().map(String::from));
    name.map(|n| !n.is_empty()).unwrap_or(true)
}

/// EMF built-in default constraints (diagnostic source `org.eclipse.emf.ecore`),
/// the Rust port of C++ `emf-ecore-util::EObjectValidator::validate_EveryDefaultConstraint`.
///
/// C++ `EValidator::validate(target, mode)` unconditionally runs this bridge in
/// *both* BATCH and LIVE modes (see `EValidator.cpp`), so a faithful Rust
/// `validate_mode` must too. The sub-checks that need model metadata absent from
/// the Rust dynamic surface (`eOpposite`, `eKeys`, enum literals, `EcoreUtil.getID`)
/// are no-ops exactly like their C++ counterparts (`validate_UniqueID`,
/// `validate_MapEntryUnique`), so the observable output matches.
///
/// Only targets backed by the dynamic model carry structural-feature metadata
/// (`structural_features_ref()`); other targets yield no diagnostics, matching
/// C++ only in the sense that C++ always has an `EClass` — the benchmark and all
/// ARXML loads go through `DynamicEObject`, so this is the exercised path.
fn emf_default_constraints(target: &dyn EObject) -> Vec<emf_common::diagnostic::Diagnostic> {
    use emf_common::diagnostic::{Diagnostic, Severity};

    /// `EObjectValidator.DIAGNOSTIC_SOURCE`.
    const SOURCE: &str = "org.eclipse.emf.ecore";
    /// `EOBJECT__EVERY_MULTIPCITY_CONFORMS`.
    const MULTIPLICITY: i32 = 1;
    /// `EOBJECT__EVERY_REFERENCE_IS_CONTAINED`.
    const REFERENCE_CONTAINED: i32 = 3;
    /// `EOBJECT__EVERY_PROXY_RESOLVES`.
    const PROXY_RESOLVES: i32 = 4;

    let mut out = Vec::new();
    let Some(dyno) = target.as_any().downcast_ref::<DynamicEObject>() else {
        return out;
    };

    // validate_EveryMultiplicityConforms → validate_MultiplicityConforms.
    for f in dyno.structural_features_ref() {
        if f.upper_bound() != 1 {
            // isMany: only checked when the feature is required (lowerBound > 0).
            //
            // C++ `validate_MultiplicityConforms` derives the value count by
            // type-matching `eGet`'s `std::any` against `std::vector<EObject*>`
            // *only*. The generated AUTOSAR getters never yield that type: a
            // many-valued reference returns an `EObjectRefView`, a many-valued
            // attribute a `std::vector<T>`. Both therefore miss the match and the
            // count reads 0, so C++ reports `_UI_FeatureHasTooFewValues_diagnostic`
            // for every required many-valued feature regardless of its contents.
            // The dynamic Rust value forms (`Val::List` of objects / scalars)
            // correspond to those same unrecognized shapes, so mirror the
            // observable C++ result exactly rather than counting the list.
            if f.lower_bound() > 0 {
                out.push(Diagnostic::new(
                    Severity::Error,
                    SOURCE,
                    MULTIPLICITY,
                    "_UI_FeatureHasTooFewValues_diagnostic",
                ));
            }
        } else if f.lower_bound() > 0 && f.is_reference() {
            // isRequired single-valued *reference*: unset *and* value-less is a
            // violation. C++ `eGet` on a scalar attribute always yields the
            // (defaulted) value, so only a reference can read as null — matching
            // Java `validate_MultiplicityConforms`, whose `eGet(...) == null` test
            // is unreachable for attributes.
            if !dyno.e_is_set(f.name()) {
                let is_null = dyno.e_get(f.name()).is_none_or(|v| matches!(v, Val::Null));
                if is_null {
                    out.push(Diagnostic::new(
                        Severity::Error,
                        SOURCE,
                        MULTIPLICITY,
                        "_UI_RequiredFeatureMustBeSet_diagnostic",
                    ));
                }
            }
        }
    }

    // validate_EveryProxyResolves / validate_EveryReferenceIsContained.
    for xr in dyno.e_cross_references() {
        let xr = xr.borrow();
        if xr.e_is_proxy() {
            out.push(Diagnostic::new(
                Severity::Error,
                SOURCE,
                PROXY_RESOLVES,
                "_UI_UnresolvedProxy_diagnostic",
            ));
        } else if dyno.e_container().is_some() && xr.e_container().is_none() {
            out.push(Diagnostic::new(
                Severity::Error,
                SOURCE,
                REFERENCE_CONTAINED,
                "_UI_DanglingReference_diagnostic",
            ));
        }
    }

    out
}

/// Default constraint evaluator: a required (`lowerBound >= 1`), single-valued
/// *reference* must not be null (C++ `noNullReqRefEval`).
///
/// Primary path: reflect over the object's dynamic `EClass` features (via the
/// `DynamicEObject` reflection surface) and, for every reference with
/// `lowerBound >= 1` that is not many-valued, check the current value is
/// non-null. This mirrors C++ walking `EClass::getEAllStructuralFeatures()`.
///
/// Fallback trade-off: the generic `EObject` surface only exposes `e_class()`
/// (a name string) — it carries no structural-feature metadata. When the target
/// is not a `DynamicEObject` (or reflection can't enumerate features), we cannot
/// discover which references are required, so we conservatively treat the object
/// as *passing* to avoid false positives. This diverges from C++, which always
/// has `EClass` metadata, at the cost of not catching the required-null case for
/// non-dynamic targets.
fn no_null_required_ref_eval(target: &dyn EObject) -> bool {
    if let Some(dyno) = target.as_any().downcast_ref::<DynamicEObject>() {
        for f in dyno.structural_features_ref() {
            if !f.is_reference() || f.is_many() {
                // Only single-valued references can be "required but null".
                continue;
            }
            if f.lower_bound() >= 1 {
                let v = dyno.e_get(f.name());
                let is_null = v.is_none() || matches!(v, Some(Val::Null));
                if is_null {
                    return false; // violation
                }
            }
        }
        return true; // every required single-valued reference is satisfied
    }
    // Fallback for non-`DynamicEObject` targets (no feature metadata): pass.
    true
}

/// `EValidator`: registers constraints and validates objects against them.
///
/// The process-wide registry (`Registry::instance`) maps package names to
/// validators, mirroring EMF's `EValidator.Registry.INSTANCE`.
#[derive(Default)]
pub struct EValidator {
    constraints: Vec<Constraint>,
    listeners: Vec<Box<dyn IConstraintListener>>,
}

impl EValidator {
    /// A new validator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a constraint (by id). If the id already exists, it is replaced.
    pub fn register_constraint(&mut self, constraint: Constraint) {
        let id = constraint.id().to_string();
        self.constraints.retain(|c| c.id() != id);
        for l in &mut self.listeners {
            l.constraint_registered(&constraint);
        }
        self.constraints.push(constraint);
    }

    /// Convenience factory: build and register a constraint from parts.
    pub fn add_constraint(
        &mut self,
        evaluator: Box<crate::constraint::Evaluator>,
        id: impl Into<String>,
        name: impl Into<String>,
        message: impl Into<String>,
        severity: Severity,
        mode: ConstraintMode,
    ) {
        self.register_constraint(Constraint::new(
            evaluator, id, name, message, severity, mode,
        ));
    }

    /// Unregister a constraint by id.
    pub fn unregister_constraint(&mut self, id: &str) -> bool {
        let before = self.constraints.len();
        self.constraints.retain(|c| c.id() != id);
        self.constraints.len() != before
    }

    /// Look up a constraint by id.
    pub fn get_constraint(&self, id: &str) -> Option<&Constraint> {
        self.constraints.iter().find(|c| c.id() == id)
    }

    /// All registered constraints.
    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    /// Add a listener.
    pub fn add_listener(&mut self, listener: Box<dyn IConstraintListener>) {
        self.listeners.push(listener);
    }

    /// Validate a single object against all registered constraints (any mode),
    /// returning the diagnostics whose severity is worse than `Ok`.
    pub fn validate(&self, target: &dyn EObject) -> Vec<emf_common::diagnostic::Diagnostic> {
        self.validate_mode(target, None)
    }

    /// Validate a single object, optionally filtered by constraint mode.
    pub fn validate_mode(
        &self,
        target: &dyn EObject,
        mode: Option<ConstraintMode>,
    ) -> Vec<emf_common::diagnostic::Diagnostic> {
        let mut out = Vec::new();
        for c in &self.constraints {
            if mode.is_some_and(|m| c.mode() != m) {
                continue;
            }
            if !c.evaluate(target) {
                out.push(emf_common::diagnostic::Diagnostic::new(
                    crate::diagnostician::map_severity(c.severity()),
                    c.name(),
                    0,
                    c.message(),
                ));
            }
        }
        // C++ `EValidator::validate` additionally runs the EMF built-in default
        // constraints (`validate_EveryDefaultConstraint`) in *both* modes,
        // independent of the registered constraints; mirror that here.
        out.extend(emf_default_constraints(target));
        out
    }

    /// Register the default built-in constraints, each in both `BATCH` and
    /// `LIVE` modes (matching C++ `registerDefaultConstraints`, whose `BATCH`
    /// and `LIVE` twins carry ids with a `.live` suffix).
    ///
    /// - `no_empty_name` (`"NoEmptyName"`): a `name` feature must not be empty.
    /// - `no_null_required_ref` (`"NoNullRequiredRef"`): any `lowerBound >= 1`
    ///   single-valued reference must not be null.
    pub fn register_default_constraints(&mut self) {
        // no_empty_name: a feature named "name" holding an empty string is invalid.
        self.add_constraint(
            Box::new(no_empty_name_eval),
            "emf.validation.default.no_empty_name",
            "NoEmptyName",
            "The name attribute must not be empty",
            Severity::Warning,
            ConstraintMode::Batch,
        );
        self.add_constraint(
            Box::new(no_empty_name_eval),
            "emf.validation.default.no_empty_name.live",
            "NoEmptyName",
            "The name attribute must not be empty",
            Severity::Warning,
            ConstraintMode::Live,
        );
        // no_null_required_ref: a required (lowerBound >= 1), single-valued
        // reference must stay set (reflection over the object's eClass).
        self.add_constraint(
            Box::new(no_null_required_ref_eval),
            "emf.validation.default.no_null_required_ref",
            "NoNullRequiredRef",
            "A required reference must not be null",
            Severity::Error,
            ConstraintMode::Batch,
        );
        self.add_constraint(
            Box::new(no_null_required_ref_eval),
            "emf.validation.default.no_null_required_ref.live",
            "NoNullRequiredRef",
            "A required reference must not be null",
            Severity::Error,
            ConstraintMode::Live,
        );
    }
}

/// Per-`EPackage` registry of validators for `Diagnostician` dispatch.
#[derive(Default)]
pub struct Registry {
    map: Vec<(String, EValidator)>,
}

impl Registry {
    /// A fresh, empty registry.
    pub fn new() -> Self {
        Self { map: Vec::new() }
    }

    /// Build a registry from an iterator of package→validator pairs.
    pub fn from_pairs<I>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (&'static str, EValidator)>,
    {
        Self {
            map: pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        }
    }

    /// Associate a validator with a package (by ns-URI).
    pub fn put(&mut self, package: &str, validator: EValidator) {
        if let Some(entry) = self.map.iter_mut().find(|(k, _)| k == package) {
            entry.1 = validator;
        } else {
            self.map.push((package.to_string(), validator));
        }
    }

    /// Look up a validator for a package.
    pub fn get(&self, package: &str) -> Option<&EValidator> {
        self.map.iter().find(|(k, _)| k == package).map(|(_, v)| v)
    }

    /// Whether a package has a validator.
    pub fn contains_key(&self, package: &str) -> bool {
        self.map.iter().any(|(k, _)| k == package)
    }

    /// Remove a package's validator.
    pub fn remove(&mut self, package: &str) {
        self.map.retain(|(k, _)| k != package);
    }
}

/// Convenience: validate a single object using the registry-resolved validator
/// for its class package (falling back to global registry default).
pub fn validate(
    target: &dyn EObject,
    registry: &Registry,
) -> Vec<emf_common::diagnostic::Diagnostic> {
    let package = crate::diagnostician::package_of(target);
    match registry.get(&package) {
        Some(v) => v.validate(target),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_common::value::Val;

    #[test]
    fn register_validate_violations() {
        let mut v = EValidator::new();
        v.add_constraint(
            Box::new(|_| false),
            "always_fail",
            "Always Fail",
            "violated",
            Severity::Error,
            ConstraintMode::Batch,
        );
        v.add_constraint(
            Box::new(|_| true),
            "always_pass",
            "Always Pass",
            "ok",
            Severity::Info,
            ConstraintMode::Batch,
        );
        let item = crate::test_util::make_item();
        let diags = v.validate(&*item.borrow());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message(), "violated");
        assert_eq!(diags[0].severity(), emf_common::diagnostic::Severity::Error);
    }

    #[test]
    fn registry_put_get_by_package() {
        let mut r = Registry { map: Vec::new() };
        let v = EValidator::new();
        r.put("http://example/ecore", v);
        assert!(r.contains_key("http://example/ecore"));
        assert!(r.get("http://example/ecore").is_some());
        r.remove("http://example/ecore");
        assert!(!r.contains_key("http://example/ecore"));
    }

    #[test]
    fn default_constraint_flags_empty_name() {
        let mut v = EValidator::new();
        v.register_default_constraints();
        let item = crate::test_util::make_item();
        let diags = v.validate(&*item.borrow());
        assert_eq!(diags.len(), 0); // no "name" set -> not flagged
        item.borrow_mut().e_set("name", Val::string(""));
        let diags = v.validate(&*item.borrow());
        // `register_default_constraints` registers no_empty_name in both BATCH
        // and LIVE modes; `validate` (any mode) runs both twins -> 2 diagnostics.
        assert_eq!(diags.len(), 2);
        // Diagnostic source reflects the constraint name (C++ getName()).
        assert!(diags.iter().all(|d| d.source() == "NoEmptyName"));
        assert!(diags
            .iter()
            .all(|d| d.message() == "The name attribute must not be empty"));
    }
}
