//! # emf-ecore
//!
//! The Ecore metamodel types, ported from the C++ `emf-ecore` module of
//! `hebin123456/artop-cpp` (aligned to Java `org.eclipse.emf.ecore`).
//!
//! This crate provides the *runtime metamodel* every generated model builds on:
//! `EClass`, `EStructuralFeature`/`EAttribute`/`EReference`, `EOperation`,
//! `EPackage`, `EFactory`, `EEnum`/`EEnumLiteral`, `EDataType`, the built-in
//! data types of [`EcorePackage`], and [`DynamicEObject`] — a reflective object
//! that stores feature values keyed by `FeatureID` and answers `eGet`/`eSet`/
//! `eIsSet`/`eUnset`.
//!
//! Design notes (matching the argument in `emf-common`):
//! - Object identity and references are `Rc<RefCell<dyn EObject>>`
//!   ([`emf_common::value::ObjectRef`]); feature *values* are the type-safe
//!   [`Val`] envelope.
//! - Inheritance is metadata, not Rust subtyping: `EClass::e_super_types`
//!   names parent classes and [`EClass`] walks them to give `e_all_features`,
//!   `e_all_attributes`, `is_super_type_of`, etc.
//! - Features are assigned integer [`FeatureID`]s by their declaring package,
//!   exactly like EMF's `featureID`; `DynamicEObject` indexes its storage on
//!   those integers.
//!
//! The generated AUTOSAR registry (`autosar448-model`) and `emf-xmi` load a
//! model by constructing `EClass` descriptors here and then instantiating
//! `DynamicEObject`s through `EcorePackage`'s factory.

/// FeatureID constants for the Ecore meta-meta-model (C++ `FeatureID`).
pub mod feature_id;

/// `EAnnotation` — a lightweight key/value annotation on a model element.
pub mod annotation;

/// `EClassifier`, `EClass`, `EDataType`, `EEnum`, `EEnumLiteral`.
pub mod classifier;

/// `EStructuralFeature`, `EAttribute`, `EReference`, `EOperation`, `EParameter`.
pub mod structural;

/// `EPackage`, `EPackageRegistry`, `EFactory`.
pub mod package;

/// `DynamicEObject` — a reflective object backed by an `EClass`.
pub mod dynamic;

/// Ecore built-in data type helpers (create from / convert to string).
pub mod datatype;

/// The singleton `EcorePackage` with built-in data types and meta-classes.
pub mod ecore_package;

pub use annotation::EAnnotation;
pub use classifier::{EClass, EClassKind, EClassifier, EDataType, EEnum, EEnumLiteral};
pub use dynamic::{adopt_many, adopt_single, node_to_object, DynNode, DynamicEObject};
pub use ecore_package::{ECORE_NS_PREFIX, ECORE_NS_URI};
pub use feature_id::FeatureID;
pub use package::{make_package_ref, EFactory, EPackage, PackageRef, PackageRegistry};
pub use structural::{EAttribute, EOperation, EParameter, EReference, EStructuralFeature};

/// Alias for the shared value type (re-exported for crate ergonomics).
pub use emf_common::value::{ObjectRef, Val};
