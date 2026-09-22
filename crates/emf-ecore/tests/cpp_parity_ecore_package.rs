//! C++ parity suite: emf-ecore EcorePackage.
//!
//! Ports `EcorePackageTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-ecore/tests/` against Rust `ecore_package`
//! (the built-in Ecore meta-meta-model singleton) and `FeatureID` constants.
//!
//! Notes tracked in PARITY_TRACKER:
//!   - Rust exposes data types / classes via `find_data_type` / `find_class`
//!     instead of typed getters like `getEClass_EClass`; behavior is checked
//!     through lookup-by-name.
//!   - `ECORE_NS_URI` / `ECORE_NS_PREFIX` / `ECORE_NAME` and the `FeatureID`
//!     block are mirrored from C++.
use emf_ecore::ecore_package::{ecore_package, ECORE_NAME, ECORE_NS_PREFIX, ECORE_NS_URI};
use emf_ecore::{FeatureID, PackageRegistry};

#[test]
fn ecore_package_initialize() {
    let pkg = ecore_package();
    assert!(pkg.borrow().classes().len() >= 19);
    assert!(pkg.borrow().data_types().len() >= 18);
}

#[test]
fn meta_classes_non_null() {
    let pkg = ecore_package();
    for name in ["EClass", "EAttribute", "EReference", "EPackage", "EEnum", "EDataType"] {
        let guard = pkg.borrow();
        let cls = guard.find_class(name).expect("meta-class registered");
        assert_eq!(cls.name(), name);
    }
}

#[test]
fn builtin_data_types_non_null() {
    let pkg = ecore_package();
    for (name, expected) in [
        ("EString", "EString"),
        ("EBoolean", "EBoolean"),
        ("EInt", "EInt"),
    ] {
        let guard = pkg.borrow();
        let dt = guard.find_data_type(name).expect("data type registered");
        assert_eq!(dt.name(), expected);
    }
}

#[test]
fn registered_to_global_registry() {
    let pkg = ecore_package();
    // The factory namespace URI / prefix must show up in the global registry.
    let mut reg = PackageRegistry::new();
    reg.register(pkg);
    assert!(reg.contains_key(ECORE_NS_URI));
    assert!(reg.contains_key(ECORE_NS_PREFIX));
}

#[test]
fn namespace_constants() {
    assert_eq!(ECORE_NS_URI, "http://www.eclipse.org/emf/2002/Ecore");
    assert_eq!(ECORE_NS_PREFIX, "ecore");
    assert_eq!(ECORE_NAME, "ecore");
}

#[test]
fn feature_id_constants() {
    assert_eq!(FeatureID::ECLASS_ESUPERTYPES, 4002);
    assert_eq!(FeatureID::ECLASS_ESTRUCTURALFEATURES, 4003);
    assert_eq!(FeatureID::EPACKAGE_ECLASSIFIERS, 10002);
    assert_eq!(FeatureID::EPACKAGE_ENSURI, 10000);
}