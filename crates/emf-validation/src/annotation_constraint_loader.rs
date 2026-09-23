//! `AnnotationConstraintLoader` — load OCL / named-constraints from `EClass`
//! annotations and register them into an `EValidator`.
//!
//! Port of C++ `emf-validation/src/AnnotationConstraintLoader.cpp`
//! (aligned to Java `org.eclipse.emf.ecore.EValidator` / `EObjectValidator` and
//! `EcoreValidator`). It is part of the generic framework and has no AUTOSAR
//! dependency.
//!
//! Constraint sources understood by this loader:
//!
//! 1. OCL annotation (`source == OCL_SOURCE`): each detail `(name, expr)` is an
//!    invariant; the expression is compiled by [`crate::constraint_parser::compile`]
//!    and registered under the id `"{OCL_SOURCE}#{name}"`.
//! 2. Named-constraints annotation (`source == CONSTRAINTS_SOURCE`): each detail
//!    value is a whitespace-separated list of invariant names (the key is usually
//!    the EClass name). Each named invariant is looked up in the same class's OCL
//!    annotation and registered under the id `"{CONSTRAINTS_SOURCE}#{name}"`.
//! 3. `load_all` first loads all OCL constraints, then the named subset.
//!
//! Unlike the C++ port (which keeps a per-`EClass` compile cache), this Rust
//! port compiles and registers on each call — the loader is idempotent because
//! [`EValidator::register_constraint`] replaces any same-id constraint.

use crate::constraint::{Constraint, ConstraintMode, Severity};
use crate::constraint_parser;
use crate::e_validator::EValidator;
use emf_ecore::{EClass, EAnnotation};

/// Ecore OCL annotation source URI (aligned to Java `EAnnotation.source`).
pub const OCL_SOURCE: &str = "http://www.eclipse.org/emf/2002/Ecore/OCL";

/// Ecore named-constraints annotation source URI.
pub const CONSTRAINTS_SOURCE: &str = "http://www.eclipse.org/emf/2002/Ecore/Constraints";

/// Find the annotation with the given `source` on `e_class`, if any.
fn find_annotation<'a>(e_class: &'a EClass, source: &str) -> Option<&'a EAnnotation> {
    e_class.e_annotations().iter().find(|a| a.source() == source)
}

/// Compile a single expression into a `Constraint` and register it, mirroring
/// the C++ `registerCompiled`/`compileOne` construction.
fn register_one(
    validator: &mut EValidator,
    name: &str,
    expr: &str,
    source: &str,
) -> bool {
    if name.is_empty() || expr.is_empty() {
        return false;
    }
    let constraint = Constraint::new(
        constraint_parser::compile(expr),
        format!("{source}#{name}"), // id
        name.to_string(),
        format!("constraint '{}' violated (expr: {expr})", name),
        Severity::Error,
        ConstraintMode::Batch,
    );
    validator.register_constraint(constraint);
    true
}

/// Load all OCL constraints from `e_class`'s OCL annotation into `validator`.
///
/// For each non-empty `(name, expr)` detail on the OCL annotation, compiles the
/// expression and registers it under the id `"{OCL_SOURCE}#{name}"`. Returns the
/// number of constraints registered.
pub fn load_ocl_constraints(validator: &mut EValidator, e_class: &EClass) -> usize {
    let Some(ann) = find_annotation(e_class, OCL_SOURCE) else {
        return 0;
    };
    let mut count = 0;
    for (name, expr) in ann.details() {
        if register_one(validator, name, expr, OCL_SOURCE) {
            count += 1;
        }
    }
    count
}

/// Load named constraints: read the invariant names declared by the
/// CONSTRAINTS_SOURCE annotation, then register their matching expressions from
/// the same class's OCL annotation.
///
/// Returns the number of constraints registered.
pub fn load_named_constraints(validator: &mut EValidator, e_class: &EClass) -> usize {
    let Some(ann) = find_annotation(e_class, CONSTRAINTS_SOURCE) else {
        return 0;
    };
    // Collect the whitespace-separated invariant names declared in details.
    let mut wanted: Vec<String> = Vec::new();
    for (_key, value) in ann.details() {
        for name in value.split_whitespace() {
            if !name.is_empty() && !wanted.iter().any(|w| w == name) {
                wanted.push(name.to_string());
            }
        }
    }
    if wanted.is_empty() {
        return 0;
    }
    let Some(ocl) = find_annotation(e_class, OCL_SOURCE) else {
        return 0;
    };
    let mut count = 0;
    for (name, expr) in ocl.details() {
        if !wanted.iter().any(|w| w == name) {
            continue;
        }
        if register_one(validator, name, expr, CONSTRAINTS_SOURCE) {
            count += 1;
        }
    }
    count
}

/// Load all constraints: first all OCL constraints, then the named subset.
/// Returns the total number of constraints registered.
pub fn load_all(validator: &mut EValidator, e_class: &EClass) -> usize {
    load_ocl_constraints(validator, e_class) + load_named_constraints(validator, e_class)
}