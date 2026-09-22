//! Integration tests porting the C++ `emf-ecore` dynamic-object unit tests
//! (`DynamicEObjectImplTests.cpp`, plus the *dynamic* parts of
//! `BasicEObjectTests.cpp`: eGet/eSet/eIsSet/eUnset/EContainer/EClass).
//!
//! The C++ reference drives `eGet`/`eSet`/`eIsSet`/`eUnset` by `FeatureID`
//! index. The Rust `DynamicEObject` operates by feature *name*, and the
//! FeatureID is assigned via `EStructuralFeature::set_feature_id`. This port
//! assigns the same IDs (0/1/...) that C++ uses, then exercises the features by
//! their names as the equivalent on-disk storage keys.
//!
//! "Null feature" in C++ (`eGet(nullptr)`/`eIsSet(nullptr)`/...) has no
//! C++-style `nullptr` in the name-based Rust API; it is translated to an
//! unknown feature name, which the Rust API resolves to `None`/`false` (empty).
//!
//! Capabilities that the current Rust `DynamicEObject` cannot faithfully
//! express are NOT forced here (marked 未实现 in the accompanying mapping):
//!   - BasicEObject_EClass_DefaultNull (a `DynamicEObject` always requires a
//!     class at construction; there is no nullable-class state).
//!   - BasicEObject_EContainer_AfterSetEContainer (`DynamicEObject` owns no
//!     container state and the `EObject` trait provides no way to set one).

use emf_common::eobject::EObject;
use emf_ecore::*;

/// Model mirroring the C++ `NodeModel` used in the reference tests:
///   - name  : EString (single, featureID 0)
///   - value : EInt    (single, featureID 1)
fn node_class() -> EClass {
    let mut node = EClass::new("Node", EClassKind::Class);

    let mut name = EStructuralFeature::attribute("name");
    name.set_feature_id(0);
    name.set_type_name("EString");
    node.add_feature(name);

    let mut value = EStructuralFeature::attribute("value");
    value.set_feature_id(1);
    value.set_type_name("EInt");
    node.add_feature(value);

    node
}

/// Class with two distinct single attributes `a` (EString) and `b` (EInt),
/// mirroring the `BasicEObject_EDynamicSet_DistinctFeatures` model.
fn two_attr_class() -> EClass {
    let mut cls = EClass::new("Two", EClassKind::Class);

    let mut a = EStructuralFeature::attribute("a");
    a.set_feature_id(0);
    a.set_type_name("EString");
    cls.add_feature(a);

    let mut b = EStructuralFeature::attribute("b");
    b.set_feature_id(1);
    b.set_type_name("EInt");
    cls.add_feature(b);

    cls
}

mod dynamic_eobject {
    use super::*;

    // ===== eClass =====

    #[test]
    fn eclass_returns_constructor_class() {
        // DynamicEObject_EClass_ReturnsConstructorClass
        let cls = node_class();
        let obj = DynamicEObject::new(cls);
        assert_eq!(obj.class().name(), "Node");
        assert_eq!(obj.e_class(), "Node");
    }

    // The BasicEObject counterpart: the dynamic object always returns the class
    // it was constructed with. (BasicEObject_EClass_DefaultNull is 未实现: Rust
    // DynamiceObject has no nullable-class state.)
    #[test]
    fn b_eclass_returns_set_class() {
        // BasicEObject_EClass_ReturnsSetClass
        let cls = node_class();
        let obj = DynamicEObject::new(cls);
        assert_eq!(obj.class().name(), "Node");
    }

    // ===== eSet / eGet =====

    #[test]
    fn eset_eget_string_attribute() {
        // DynamicEObject_ESetEGet_StringAttribute
        let cls = node_class();
        let mut obj = DynamicEObject::new(cls);
        assert!(obj.e_set_by_name("name", Val::String("root".into())));
        let v = obj.e_get_by_name("name").unwrap();
        assert_eq!(v.as_str(), Some("root"));
    }

    #[test]
    fn eset_eget_int_attribute() {
        // DynamicEObject_ESetEGet_IntAttribute
        let cls = node_class();
        let mut obj = DynamicEObject::new(cls);
        assert!(obj.e_set_by_name("value", Val::Int(42)));
        let v = obj.e_get_by_name("value").unwrap();
        assert_eq!(v.as_int(), Some(42));
    }

    #[test]
    fn eset_eget_by_feature_id() {
        // DynamicEObject_ESetEGet_ByFeatureID
        // C++: eSet(0, "byID") / eGet(0); featureID 0 == feature "name".
        let cls = node_class();
        assert_eq!(
            cls.feature_id_of("name", &ecore_package::global()).unwrap(),
            0
        );
        let mut obj = DynamicEObject::new(cls);
        assert!(obj.e_set_by_name("name", Val::String("byID".into())));
        let v = obj.e_get_by_name("name").unwrap();
        assert_eq!(v.as_str(), Some("byID"));
    }

    #[test]
    fn eset_eget_by_feature_id_int() {
        // DynamicEObject_ESetEGet_ByFeatureID_Int
        // C++: eSet(1, 99) / eGet(1); featureID 1 == feature "value".
        let cls = node_class();
        assert_eq!(
            cls.feature_id_of("value", &ecore_package::global())
                .unwrap(),
            1
        );
        let mut obj = DynamicEObject::new(cls);
        assert!(obj.e_set_by_name("value", Val::Int(99)));
        let v = obj.e_get_by_name("value").unwrap();
        assert_eq!(v.as_int(), Some(99));
    }

    #[test]
    fn eget_unset_attribute_empty() {
        // DynamicEObject_EGet_UnsetAttribute_Empty
        // A never-set attribute reads back as null/empty.
        let cls = node_class();
        let obj = DynamicEObject::new(cls);
        let v = obj.e_get_by_name("name").unwrap();
        assert!(v.is_null());
    }

    #[test]
    fn eget_null_feature_empty() {
        // DynamicEObject_EGet_NullFeature_Empty
        // C++ eGet(nullptr) -> empty. Unknown feature name -> None.
        let cls = node_class();
        let obj = DynamicEObject::new(cls);
        assert!(obj.e_get_by_name("no_such_feature").is_none());
    }

    #[test]
    fn eget_with_resolve_flag() {
        // DynamicEObject_EGet_WithResolveFlag
        // C++ eGet(feature, resolve) delegates to the no-arg eGet.
        // Rust has a single read path; the flag is implicit.
        let cls = node_class();
        let mut obj = DynamicEObject::new(cls);
        assert!(obj.e_set_by_name("name", Val::String("x".into())));
        let v = obj.e_get_by_name("name").unwrap();
        assert_eq!(v.as_str(), Some("x"));
    }

    #[test]
    fn eset_overwrite_attribute() {
        // DynamicEObject_ESet_OverwriteAttribute
        let cls = node_class();
        let mut obj = DynamicEObject::new(cls);
        assert!(obj.e_set_by_name("name", Val::String("first".into())));
        assert!(obj.e_set_by_name("name", Val::String("second".into())));
        let v = obj.e_get_by_name("name").unwrap();
        assert_eq!(v.as_str(), Some("second"));
    }

    #[test]
    fn eset_null_feature_no_op() {
        // DynamicEObject_ESet_NullFeature_NoOp
        // C++ eSet(nullptr, ...) is a no-op; unknown name -> false, no crash.
        let cls = node_class();
        let mut obj = DynamicEObject::new(cls);
        assert!(!obj.e_set_by_name("no_such_feature", Val::String("x".into())));
        // The object remains usable.
        assert!(obj.e_get_by_name("name").unwrap().is_null());
    }

    // ===== eIsSet / eUnset =====

    #[test]
    fn eis_set_default_false() {
        // DynamicEObject_EIsSet_DefaultFalse
        let cls = node_class();
        let obj = DynamicEObject::new(cls);
        assert!(!obj.e_is_set_by_name("name").unwrap());
        assert!(!obj.e_is_set_by_name("value").unwrap());
    }

    #[test]
    fn eis_set_after_set_true() {
        // DynamicEObject_EIsSet_AfterSetTrue
        let cls = node_class();
        let mut obj = DynamicEObject::new(cls);
        assert!(obj.e_set_by_name("name", Val::String("x".into())));
        assert!(obj.e_is_set_by_name("name").unwrap());
    }

    #[test]
    fn eis_set_null_feature_false() {
        // DynamicEObject_EIsSet_NullFeature_False
        // C++ eIsSet(nullptr) -> false; unknown name -> None (-> false).
        let cls = node_class();
        let obj = DynamicEObject::new(cls);
        assert!(!obj.e_is_set_by_name("no_such_feature").unwrap_or(false));
    }

    #[test]
    fn eis_set_by_feature_id() {
        // DynamicEObject_EIsSet_ByFeatureID
        // C++: eIsSet(0) false, eSet(0, "y"), eIsSet(0) true (featureID 0 == name).
        let cls = node_class();
        assert_eq!(
            cls.feature_id_of("name", &ecore_package::global()).unwrap(),
            0
        );
        let mut obj = DynamicEObject::new(cls);
        assert!(!obj.e_is_set_by_name("name").unwrap());
        assert!(obj.e_set_by_name("name", Val::String("y".into())));
        assert!(obj.e_is_set_by_name("name").unwrap());
    }

    #[test]
    fn eunset_clears_value() {
        // DynamicEObject_EUnset_ClearsValue
        let cls = node_class();
        let mut obj = DynamicEObject::new(cls);
        assert!(obj.e_set_by_name("name", Val::String("x".into())));
        assert!(obj.e_is_set_by_name("name").unwrap());
        assert!(obj.e_unset_by_name("name"));
        assert!(!obj.e_is_set_by_name("name").unwrap());
        assert!(obj.e_get_by_name("name").unwrap().is_null());
    }

    #[test]
    fn eunset_by_feature_id() {
        // DynamicEObject_EUnset_ByFeatureID
        // C++: eSet(1, 7), eIsSet(1) true, eUnset(1), eIsSet(1) false (id 1 == value).
        let cls = node_class();
        assert_eq!(
            cls.feature_id_of("value", &ecore_package::global())
                .unwrap(),
            1
        );
        let mut obj = DynamicEObject::new(cls);
        assert!(obj.e_set_by_name("value", Val::Int(7)));
        assert!(obj.e_is_set_by_name("value").unwrap());
        assert!(obj.e_unset_by_name("value"));
        assert!(!obj.e_is_set_by_name("value").unwrap());
    }

    #[test]
    fn eunset_null_feature_no_op() {
        // DynamicEObject_EUnset_NullFeature_NoOp
        // C++ eUnset(nullptr) is a no-op; unknown name -> false, no crash.
        let cls = node_class();
        let mut obj = DynamicEObject::new(cls);
        assert!(!obj.e_unset_by_name("no_such_feature"));
    }

    // ===== eContainer =====

    #[test]
    fn b_econtainer_default_null() {
        // BasicEObject_EContainer_DefaultNull
        // The EObject trait default e_container() returns None; DynamicEObject
        // does not override it, so a fresh object has no container.
        let cls = node_class();
        let obj = DynamicEObject::new(cls);
        assert!(obj.e_container().is_none());
    }

    // BasicEObject_EContainer_AfterSetEContainer is 未实现:
    // DynamicEObject owns no container state and offers no way to set one.

    // ===== eDynamic* (BasicEObject dynamic-value API) =====

    #[test]
    fn edynamic_get_unset_returns_empty() {
        // BasicEObject_EDynamicGet_Unset_ReturnsEmpty
        let cls = node_class();
        let obj = DynamicEObject::new(cls);
        let v = obj.e_get_by_name("name").unwrap();
        assert!(v.is_null());
    }

    #[test]
    fn edynamic_set_then_get() {
        // BasicEObject_EDynamicSet_ThenGet
        let cls = node_class();
        let mut obj = DynamicEObject::new(cls);
        assert!(obj.e_set_by_name("name", Val::String("Alice".into())));
        let v = obj.e_get_by_name("name").unwrap();
        assert_eq!(v.as_str(), Some("Alice"));
    }

    #[test]
    fn edynamic_set_distinct_features() {
        // BasicEObject_EDynamicSet_DistinctFeatures
        let cls = two_attr_class();
        let mut obj = DynamicEObject::new(cls);
        assert!(obj.e_set_by_name("a", Val::String("hello".into())));
        assert!(obj.e_set_by_name("b", Val::Int(42)));
        let va = obj.e_get_by_name("a").unwrap();
        let vb = obj.e_get_by_name("b").unwrap();
        assert_eq!(va.as_str(), Some("hello"));
        assert_eq!(vb.as_int(), Some(42));
    }

    #[test]
    fn edynamic_set_overwrite() {
        // BasicEObject_EDynamicSet_Overwrite
        let cls = node_class();
        let mut obj = DynamicEObject::new(cls);
        assert!(obj.e_set_by_name("name", Val::String("first".into())));
        assert!(obj.e_set_by_name("name", Val::String("second".into())));
        let v = obj.e_get_by_name("name").unwrap();
        assert_eq!(v.as_str(), Some("second"));
    }

    #[test]
    fn edynamic_is_set_default_false() {
        // BasicEObject_EDynamicIsSet_DefaultFalse
        let cls = node_class();
        let obj = DynamicEObject::new(cls);
        assert!(!obj.e_is_set_by_name("name").unwrap());
    }

    #[test]
    fn edynamic_is_set_after_set_true() {
        // BasicEObject_EDynamicIsSet_AfterSetTrue
        let cls = node_class();
        let mut obj = DynamicEObject::new(cls);
        assert!(obj.e_set_by_name("name", Val::String("x".into())));
        assert!(obj.e_is_set_by_name("name").unwrap());
    }

    #[test]
    fn edynamic_unset_clears_value() {
        // BasicEObject_EDynamicUnset_ClearsValue
        let cls = node_class();
        let mut obj = DynamicEObject::new(cls);
        assert!(obj.e_set_by_name("name", Val::String("x".into())));
        assert!(obj.e_is_set_by_name("name").unwrap());
        assert!(obj.e_unset_by_name("name"));
        assert!(!obj.e_is_set_by_name("name").unwrap());
        assert!(obj.e_get_by_name("name").unwrap().is_null());
    }

    #[test]
    fn edynamic_unset_not_previously_set_no_op() {
        // BasicEObject_EDynamicUnset_NotPreviouslySet_NoOp
        let cls = node_class();
        let mut obj = DynamicEObject::new(cls);
        assert!(obj.e_unset_by_name("name")); // known feature, no prior value
        assert!(!obj.e_is_set_by_name("name").unwrap());
    }
}
