//! Model validation, batch + live (port of C++ `emf-validation`).
//!
//! Port target: C++ `emf-validation` module of `hebin123456/artop-cpp`.
//!
//! Implemented over the EMF reflection surface so it stays artop-agnostic:
//! [`constraint::Constraint`] holds an evaluator, [`e_validator::EValidator`]
//! registers constraints and validates objects, [`diagnostician::Diagnostician`]
//! walks containment trees and dispatches per-package, and
//! [`constraint_descriptor`] parses the standard XML constraint subset.

pub mod constraint;
pub mod constraint_descriptor;
pub mod diagnostician;
pub mod e_validator;

pub mod annotation_constraint_loader {
    //! Port target: C++ source unit for `annotation_constraint_loader`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-validation::annotation_constraint_loader"
    }
}

pub mod autosar_constraints {
    //! Port target: C++ source unit for `autosar_constraints`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-validation::autosar_constraints"
    }
}

pub mod constraint_parser {
    //! Port target: C++ source unit for `constraint_parser`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-validation::constraint_parser"
    }
}

pub mod live_validator;

pub mod validation_service;

/// Test-only helpers shared across unit and integration tests.
#[doc(hidden)]
pub mod test_util {
    use emf_common::value::ObjectRef;
    use emf_ecore::{make_package_ref, DynamicEObject, EClass, EClassKind, PackageRegistry};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Register a tiny "Item" package and return a class with a `name` feature.
    pub fn registry() -> PackageRegistry {
        let mut pkg = emf_ecore::EPackage::new("d");
        pkg.set_ns_prefix("d");
        let mut item = EClass::new("Item", EClassKind::Class);
        let mut name = emf_ecore::EStructuralFeature::attribute("name");
        name.set_type_name("EString");
        item.add_feature(name);
        pkg.add_class(item);
        let mut r = PackageRegistry::new();
        r.register(make_package_ref(pkg));
        r
    }

    /// An `Item` handle.
    pub fn make_item() -> ObjectRef {
        let reg = registry();
        let cls = reg.find_class("Item").unwrap();
        Rc::new(RefCell::new(DynamicEObject::new(cls)))
    }

    /// The dynamic class used by constraint tests.
    pub fn item_class() -> EClass {
        registry().find_class("Item").unwrap()
    }
}
