//! `EObjectValidator` — structural validation over `EObject` metadata.
//!
//! Port of C++ `emf-ecore-util/EObjectValidator` (aligned to Java
//! `org.eclipse.emf.ecore.util.EObjectValidator`).
//!
//! Everything here is generic EMF: it validates *metadata* (`EPackage` /
//! `EClass` / `EAttribute` / `EReference` / `EOperation`) purely through the
//! descriptive API in `emf-ecore`, with no knowledge of any domain metamodel
//! (see the decoupling principle in `docs/PROGRESS.md`).
//!
//! The API is shaped after the EMF vocabulary:
//! `validate_EveryDefaultConstraint` folds the default structural checks
//! (required `name`, and for packages a non-empty nsURI / nsPrefix) into a
//! [`DiagnosticChain`]. Concrete `validateEPackage` / `validateEClass` /
//! `validateEAttribute` / `validateEReference` / `validateEOperation` entry
//! points return the collected diagnostics.

use emf_common::diagnostic::{Diagnostic, DiagnosticChain, Severity};
use emf_ecore::{EAttribute, EClass, EEnum, EOperation, EPackage, EReference, EStructuralFeature};

/// The diagnostic source identifier, aligned to EMF
/// (`org.eclipse.emf.ecore`).
pub const DIAGNOSTIC_SOURCE: &str = "org.eclipse.emf.ecore";

/// EMF `EObjectValidator` diagnostic codes.
pub mod codes {
    /// `EObject.eEveryMultiplicityConforms`.
    pub const EOBJECT_EVERY_MULTIPLICITY_CONFORMS: i32 = 1;
    /// `EObject.eEveryDataValueConforms`.
    pub const EOBJECT_EVERY_DATA_VALUE_CONFORMS: i32 = 2;
    /// `EObject.eEveryReferenceIsContained`.
    pub const EOBJECT_EVERY_REFERENCE_IS_CONTAINED: i32 = 3;
    /// `EObject.eEveryProxyResolves`.
    pub const EOBJECT_EVERY_PROXY_RESOLVES: i32 = 4;
    /// `EObject.eNoCircularContainment`.
    pub const EOBJECT_NO_CIRCULAR_CONTAINMENT: i32 = 15;
    /// `EObject.eEveryDefaultConstraint` (folds all default checks).
    pub const EOBJECT_EVERY_DEFAULT_CONSTRAINT: i32 = 0;
}

/// A foldable structural validator. It owns no state and can be shared.
#[derive(Default, Debug, Clone)]
pub struct EObjectValidator;

impl EObjectValidator {
    /// Fold every default constraint over an object's class name, writing into
    /// `chain`. Returns `true` if everything is valid (no error-or-worse).
    pub fn validate_every_default_constraint(
        &self,
        class_name: &str,
        chain: &mut DiagnosticChain,
    ) -> bool {
        if class_name.is_empty() {
            chain.add(Diagnostic::new(
                Severity::Error,
                DIAGNOSTIC_SOURCE,
                codes::EOBJECT_EVERY_DEFAULT_CONSTRAINT,
                "The class has no name",
            ));
            false
        } else {
            true
        }
    }

    /// Validate an `EPackage` and its classifiers.
    pub fn validate_epackage(&self, pkg: &EPackage) -> Vec<Diagnostic> {
        let mut chain = DiagnosticChain::new();
        if pkg.name().is_empty() {
            chain.add(Diagnostic::new(
                Severity::Error,
                DIAGNOSTIC_SOURCE,
                codes::EOBJECT_EVERY_DEFAULT_CONSTRAINT,
                "The package has no name",
            ));
        }
        if pkg.ns_uri().is_none() || pkg.ns_uri().is_none_or(|u| u.to_string().is_empty()) {
            chain.add(Diagnostic::new(
                Severity::Error,
                DIAGNOSTIC_SOURCE,
                codes::EOBJECT_EVERY_DEFAULT_CONSTRAINT,
                "The package has no nsURI",
            ));
        }
        if pkg.ns_prefix().is_empty() {
            chain.add(Diagnostic::new(
                Severity::Error,
                DIAGNOSTIC_SOURCE,
                codes::EOBJECT_EVERY_DEFAULT_CONSTRAINT,
                "The package has no nsPrefix",
            ));
        }
        // Recurse into classifiers.
        let mut nested = DiagnosticChain::new();
        for c in pkg.classes() {
            let _ = self.validate_eclass(c, &mut nested);
        }
        for e in pkg.enums() {
            validate_eenum(e, &mut nested);
        }
        for d in nested.get() {
            chain.add(d.clone());
        }
        chain.get().to_vec()
    }

    /// Validate an `EClass` and its features.
    pub fn validate_eclass(&self, class: &EClass, chain: &mut DiagnosticChain) -> bool {
        let _ = self.validate_every_default_constraint(class.name(), chain);
        let mut ok = chain.worst().is_none_or(|s| s < Severity::Error);
        for f in class.e_structural_features() {
            ok = self.validate_feature(f, chain) && ok;
        }
        ok
    }

    /// Validate an attribute feature.
    pub fn validate_eattribute(&self, attr: &EAttribute, chain: &mut DiagnosticChain) -> bool {
        self.validate_feature(attr.feature(), chain)
    }

    /// Validate a reference feature.
    pub fn validate_ereference(&self, refe: &EReference, chain: &mut DiagnosticChain) -> bool {
        self.validate_feature(refe.feature(), chain)
    }

    /// Validate an operation.
    pub fn validate_eoperation(&self, op: &EOperation, chain: &mut DiagnosticChain) -> bool {
        if op.name().is_empty() {
            chain.add(Diagnostic::new(
                Severity::Error,
                DIAGNOSTIC_SOURCE,
                codes::EOBJECT_EVERY_DEFAULT_CONSTRAINT,
                "The operation has no name",
            ));
            return false;
        }
        true
    }

    /// Structural checks for a feature: a feature must carry a name.
    fn validate_feature(&self, feature: &EStructuralFeature, chain: &mut DiagnosticChain) -> bool {
        if feature.name().is_empty() {
            chain.add(Diagnostic::new(
                Severity::Error,
                DIAGNOSTIC_SOURCE,
                codes::EOBJECT_EVERY_DEFAULT_CONSTRAINT,
                "The feature has no name",
            ));
            false
        } else {
            true
        }
    }
}

/// Convenience: `validateEPackage(pkg)` returning diagnostics.
pub fn validate_epackage(pkg: &EPackage) -> Vec<Diagnostic> {
    EObjectValidator.validate_epackage(pkg)
}

/// Convenience: `validateEClass(cls)` returning diagnostics.
pub fn validate_eclass(class: &EClass) -> Vec<Diagnostic> {
    let mut chain = DiagnosticChain::new();
    let v = EObjectValidator;
    let _ = v.validate_eclass(class, &mut chain);
    chain.get().to_vec()
}

/// Convenience: `validateEAttribute(attr)` returning diagnostics.
pub fn validate_eattribute(attr: &EAttribute) -> Vec<Diagnostic> {
    let mut chain = DiagnosticChain::new();
    let v = EObjectValidator;
    let _ = v.validate_eattribute(attr, &mut chain);
    chain.get().to_vec()
}

/// Convenience: `validateEReference(ref_)` returning diagnostics.
pub fn validate_ereference(refe: &EReference) -> Vec<Diagnostic> {
    let mut chain = DiagnosticChain::new();
    let v = EObjectValidator;
    let _ = v.validate_ereference(refe, &mut chain);
    chain.get().to_vec()
}

/// Convenience: `validateEOperation(op)` returning diagnostics.
pub fn validate_eoperation(op: &EOperation) -> Vec<Diagnostic> {
    let mut chain = DiagnosticChain::new();
    let v = EObjectValidator;
    let _ = v.validate_eoperation(op, &mut chain);
    chain.get().to_vec()
}

fn validate_eenum(e: &EEnum, chain: &mut DiagnosticChain) {
    if e.name().is_empty() {
        chain.add(Diagnostic::new(
            Severity::Error,
            DIAGNOSTIC_SOURCE,
            codes::EOBJECT_EVERY_DEFAULT_CONSTRAINT,
            "The enum has no name",
        ));
    }
    if e.e_literals().is_empty() {
        chain.add(Diagnostic::new(
            Severity::Error,
            DIAGNOSTIC_SOURCE,
            codes::EOBJECT_EVERY_DEFAULT_CONSTRAINT,
            "The enum has no literals",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_ecore::{EClass, EClassKind, EPackage, EStructuralFeature};

    #[test]
    fn package_without_name_has_errors() {
        let p = EPackage::new("");
        let diags = validate_epackage(&p);
        assert!(!diags.is_empty(), "empty package must report problems");
        assert!(diags.iter().any(|d| d.severity() >= Severity::Error));
    }

    #[test]
    fn valid_package_has_no_name_error() {
        let mut p = EPackage::new("ok");
        p.set_ns_uri("http://x");
        p.set_ns_prefix("x");
        let mut c = EClass::new("Foo", EClassKind::Class);
        c.add_feature(EStructuralFeature::attribute("name"));
        p.add_class(c);
        let diags = validate_epackage(&p);
        let name_err = diags
            .iter()
            .filter(|d| d.severity() == Severity::Error && d.message().contains("name"))
            .count();
        assert_eq!(name_err, 0, "well-formed package has no name errors");
    }
}
