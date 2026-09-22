//! The `EcorePackage` singleton: the Ecore meta-meta-model with built-in data
//! types and the meta-classes whose FeatureIDs are defined in
//! [`crate::feature_id`].
//!
//! Because object handles in this crate are single-threaded
//! `Rc<RefCell<T>>`, the "global" registry is stored in a thread-local cell and
//! snapped into an owned clone on access. This keeps reflection borrow-free.

use crate::datatype;
use crate::package::{make_package_ref, EPackage, PackageRef, PackageRegistry};
use std::cell::RefCell;

/// The Ecore namespace URI (Java `EcorePackage.eNS_URI`).
pub const ECORE_NS_URI: &str = "http://www.eclipse.org/emf/2002/Ecore";
/// Ecore namespace prefix.
pub const ECORE_NS_PREFIX: &str = "ecore";
/// Ecore package name.
pub const ECORE_NAME: &str = "ecore";

thread_local! {
    /// The process-wide registry, initialized lazily per thread.
    static GLOBAL_REGISTRY: RefCell<Option<PackageRegistry>> = const { RefCell::new(None) };
    /// The built-in Ecore meta-package handle.
    static ECORE_PACKAGE: RefCell<Option<PackageRef>> = const { RefCell::new(None) };
}

/// Access the process-wide package registry (owned snapshot).
pub fn global() -> PackageRegistry {
    GLOBAL_REGISTRY.with(|cell| {
        let mut guard = cell.borrow_mut();
        if guard.is_none() {
            *guard = Some(build_registry());
        }
        guard.clone().expect("registry initialized")
    })
}

/// Access the Ecore meta-package singleton handle.
pub fn ecore_package() -> PackageRef {
    ECORE_PACKAGE.with(|cell| {
        let mut guard = cell.borrow_mut();
        if guard.is_none() {
            *guard = Some(build_ecore());
        }
        guard.clone().expect("ecore package initialized")
    })
}

fn build_ecore() -> PackageRef {
    let mut pkg = EPackage::new(ECORE_NAME);
    pkg.set_ns_uri(ECORE_NS_URI);
    pkg.set_ns_prefix(ECORE_NS_PREFIX);

    // Built-in data types.
    for (name, instance) in [
        (datatype::names::E_STRING, "java.lang.String"),
        (datatype::names::E_BOOLEAN, "boolean"),
        (datatype::names::E_BOOLEAN_OBJECT, "java.lang.Boolean"),
        (datatype::names::E_INT, "int"),
        (datatype::names::E_INTEGER_OBJECT, "java.lang.Integer"),
        (datatype::names::E_LONG, "long"),
        (datatype::names::E_LONG_OBJECT, "java.lang.Long"),
        (datatype::names::E_DOUBLE, "double"),
        (datatype::names::E_DOUBLE_OBJECT, "java.lang.Double"),
        (datatype::names::E_FLOAT, "float"),
        (datatype::names::E_FLOAT_OBJECT, "java.lang.Float"),
        (datatype::names::E_BYTE, "byte"),
        (datatype::names::E_BYTE_OBJECT, "java.lang.Byte"),
        (datatype::names::E_SHORT, "short"),
        (datatype::names::E_SHORT_OBJECT, "java.lang.Short"),
        (datatype::names::E_CHAR, "char"),
        (datatype::names::E_CHARACTER_OBJECT, "java.lang.Character"),
        (datatype::names::E_BIG_INTEGER, "java.math.BigInteger"),
        (datatype::names::E_BIG_DECIMAL, "java.math.BigDecimal"),
    ] {
        let mut dt = crate::EDataType::new(name, instance);
        dt.set_serializable(name != datatype::names::E_BIG_DECIMAL);
        pkg.add_data_type(dt);
    }

    // Meta-classes (bare descriptors; FeatureIDs live in feature_id module).
    for meta_class in [
        "EObject",
        "EModelElement",
        "ENamedElement",
        "ETypedElement",
        "EClassifier",
        "EClass",
        "EDataType",
        "EEnum",
        "EEnumLiteral",
        "EStructuralFeature",
        "EAttribute",
        "EReference",
        "EOperation",
        "EParameter",
        "EPackage",
        "EFactory",
        "ETypeParameter",
        "EGenericType",
        "EAnnotation",
    ] {
        pkg.add_class(crate::EClass::new(meta_class, crate::EClassKind::Class));
    }

    make_package_ref(pkg)
}

fn build_registry() -> PackageRegistry {
    let mut reg = PackageRegistry::new();
    reg.register(ecore_package());
    reg
}

/// Seed the globals — useful at startup so `DynamicEObject` reflection has a
/// populated registry immediately.
pub fn initialize() {
    let _ = global();
    let _ = ecore_package();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Val;

    #[test]
    fn builtin_data_types_registered() {
        let pkg = ecore_package();
        assert_eq!(pkg.borrow().ns_uri().unwrap().to_string(), ECORE_NS_URI);
        assert!(pkg.borrow().find_data_type("EString").is_some());
        assert!(pkg.borrow().find_data_type("EInt").is_some());
        assert!(pkg.borrow().find_data_type("EBoolean").is_some());
    }

    #[test]
    fn datatype_parse_roundtrip() {
        assert_eq!(datatype::from_string("EInt", "42"), Val::Int(42));
        assert_eq!(datatype::from_string("EDouble", "2.5"), Val::Double(2.5));
        assert_eq!(datatype::from_string("EBoolean", "true"), Val::Bool(true));
        assert_eq!(
            datatype::from_string("EString", "hi"),
            Val::String("hi".into())
        );
        assert_eq!(datatype::to_string("EInt", &Val::Int(7)), "7");
    }
}
