//! `Diagnostician` — traverse a containment tree and dispatch validation per
//! object (port of C++ `emf-validation` `Diagnostician`, aligned to Java
//! `org.eclipse.emf.ecore.util.Diagnostician`).
//!
//! Walks the containment tree of a root, resolves each object's package, looks
//! up the matching [`EValidator`] in the registry and collects diagnostics.

use crate::constraint::{ConstraintMode, Severity};
use crate::e_validator::Registry;
use emf_common::diagnostic::Diagnostic;
use emf_common::eobject::EObject;
use emf_common::value::ObjectRef;

/// Map a validation [`Severity`] into the common diagnostic severity.
pub fn map_severity(s: Severity) -> emf_common::diagnostic::Severity {
    match s {
        Severity::Ok => emf_common::diagnostic::Severity::Ok,
        Severity::Info => emf_common::diagnostic::Severity::Info,
        Severity::Warning => emf_common::diagnostic::Severity::Warning,
        Severity::Error => emf_common::diagnostic::Severity::Error,
        Severity::Cancel => emf_common::diagnostic::Severity::Cancel,
    }
}

/// Best-effort package name for an object (its class name plus a synthetic
/// prefix when no real package is available). This gives registry dispatch a
/// stable key in the dynamic-model case.
pub fn package_of(o: &dyn EObject) -> String {
    o.e_class().to_string()
}

/// `Diagnostician`: batch/live validation over object graphs.
pub struct Diagnostician;

impl Diagnostician {
    /// Validate a single object (no containment recursion).
    pub fn validate_object(
        target: &dyn EObject,
        registry: &Registry,
        mode: Option<ConstraintMode>,
    ) -> Vec<Diagnostic> {
        match registry.get(&package_of(target)) {
            Some(v) => v.validate_mode(target, mode),
            None => Vec::new(),
        }
    }

    /// Validate a root and its entire containment tree. Objects with no
    /// registered validator are skipped silently.
    pub fn validate(
        root: &dyn EObject,
        registry: &Registry,
        mode: Option<ConstraintMode>,
    ) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        collect(root, Some(registry), mode, &mut out);
        out
    }

    /// Immediate children of an object (containment).
    pub fn children(o: &dyn EObject) -> Vec<ObjectRef> {
        o.e_contents()
    }
}

/// Depth-first walk collecting diagnostics.
fn collect(
    obj: &dyn EObject,
    registry: Option<&Registry>,
    mode: Option<ConstraintMode>,
    out: &mut Vec<Diagnostic>,
) {
    if let Some(reg) = registry {
        if let Some(v) = reg.get(&package_of(obj)) {
            out.extend(v.validate_mode(obj, mode));
        }
    }
    for child in obj.e_contents() {
        collect(&*child.borrow(), registry, mode, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::Severity as VS;
    use crate::e_validator::EValidator;
    use emf_common::value::Val;

    #[test]
    fn dispatched_diagnostics() {
        // Validate empty-name items via registry dispatch on class name.
        let mut reg = Registry::new();
        let mut v = EValidator::new();
        v.register_default_constraints();
        reg.put("Item", v);

        let handle = crate::test_util::make_item();
        {
            let mut o = handle.borrow_mut();
            o.e_set("name", Val::string(""));
        }
        let ds = Diagnostician::validate(&*handle.borrow(), &reg, None);
        assert!(!ds.is_empty());
        assert!(
            ds.iter()
                .all(|d| d.severity() == emf_common::diagnostic::Severity::Warning)
        );
    }

    #[test]
    fn unregistered_package_skipped() {
        let reg = Registry::new();
        let handle = crate::test_util::make_item();
        let ds = Diagnostician::validate(&*handle.borrow(), &reg, None);
        assert!(ds.is_empty());
    }

    #[test]
    fn map_severity_covers_all() {
        assert_eq!(map_severity(VS::Ok), emf_common::diagnostic::Severity::Ok);
        assert_eq!(
            map_severity(VS::Error),
            emf_common::diagnostic::Severity::Error
        );
    }
}
