//! Integration tests exercising the ecore metamodel core end-to-end:
//! package building, class/feature reflection, `DynamicEObject`
//! `eGet`/`eSet`/`eIsSet`/`eUnset`, `EFactory` creation, and enum handling.

use artop_common::eobject::EObject;
use artop_ecore::*;

/// Register a small sample package: `AIdentifiable` (abstract) with a
/// `shortName`, plus `ARElement` inheriting it with an own attribute and a
/// containment reference.
fn sample_registry() -> PackageRegistry {
    let mut pkg = EPackage::new("sample");
    pkg.set_ns_uri("http://sample/1.0");
    pkg.set_ns_prefix("smp");

    let mut identifiable = EClass::new("AIdentifiable", EClassKind::AbstractClass);
    let mut short_name = EStructuralFeature::attribute("shortName");
    short_name.set_type_name("EString");
    identifiable.add_feature(short_name);

    let mut element = EClass::new("ARElement", EClassKind::Class);
    element.add_super_type("AIdentifiable").unwrap();
    let mut category = EStructuralFeature::attribute("category");
    category.set_feature_id(1);
    category.set_type_name("EString");
    element.add_feature(category);

    let mut children = EStructuralFeature::reference_many("children");
    children.set_feature_id(2);
    children.set_type_name("ARElement");
    element.add_feature(children);

    pkg.add_class(identifiable);
    pkg.add_class(element);

    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));
    reg
}

#[test]
fn class_reflection_walks_inheritance() {
    let reg = sample_registry();
    let element = reg.find_class("ARElement").unwrap();

    // Ancestor-first super-type chain.
    assert_eq!(element.e_all_super_types(&reg), ["AIdentifiable"]);
    assert!(element.is_super_type_of("AIdentifiable", &reg));
    assert!(!element.is_super_type_of("ARElement", &reg)); // not a proper ancestor of itself

    // All features = inherited shortName + own category + own children.
    let features = element.e_all_structural_features(&reg);
    let names: Vec<&str> = features.iter().map(|f| f.name()).collect();
    assert_eq!(names, ["shortName", "category", "children"]);

    // Attribute vs reference split.
    let attrs = element.e_all_attributes(&reg);
    assert_eq!(attrs.len(), 2);
    let refs = element.e_all_references(&reg);
    assert_eq!(refs.len(), 1);
    assert!(refs[0].is_reference());
}

#[test]
fn dynamic_object_get_set_unset() {
    let reg = sample_registry();
    let element = reg.find_class("ARElement").unwrap();
    let mut obj = DynamicEObject::new_in(element, reg);

    // Unset => default (null).
    assert!(!obj.e_is_set_by_name("shortName").unwrap());
    assert!(obj.e_get_by_name("shortName").is_some());
    assert!(obj.e_get_by_name("shortName").unwrap().is_null());

    // Set then read.
    assert!(obj.e_set_by_name("shortName", Val::String("MyEl".into())));
    assert!(obj.e_is_set_by_name("shortName").unwrap());
    assert_eq!(
        obj.e_get_by_name("shortName")
            .and_then(|v| v.as_str().map(String::from)),
        Some("MyEl".into())
    );

    // Unset restores the default.
    assert!(obj.e_unset_by_name("shortName"));
    assert!(!obj.e_is_set_by_name("shortName").unwrap());
}

#[test]
fn dynamic_object_reflection_trait() {
    let reg = sample_registry();
    let element = reg.find_class("ARElement").unwrap();
    let mut obj = DynamicEObject::new_in(element, reg);
    assert_eq!(obj.e_class(), "ARElement");

    // Use the EObject trait surface.
    assert!(obj.e_set("category", Val::String("powertrain".into())));
    assert!(obj.e_is_set("category"));
    let got = obj.e_get("category");
    assert_eq!(
        got.and_then(|v| v.as_str().map(String::from)),
        Some("powertrain".into())
    );
    assert!(obj.e_unset("category"));
    assert!(!obj.e_is_set("category"));
}

#[test]
fn factory_creates_and_parses() {
    let reg = sample_registry();
    let element = reg.find_class("ARElement").unwrap();

    let factory = EFactory::new();
    let obj = factory.create(&element);
    let cls = obj.borrow().e_class().to_string();
    assert_eq!(cls, "ARElement");

    // Data-type string conversion via the factory.
    assert_eq!(factory.create_from_string("EInt", "7"), Val::Int(7));
    assert_eq!(factory.convert_to_string("EInt", &Val::Int(7)), "7");
    assert_eq!(
        factory.create_from_string("EBoolean", "true"),
        Val::Bool(true)
    );
}

#[test]
fn package_registry_lookup_by_ns_uri() {
    let mut pkg = EPackage::new("sample");
    pkg.set_ns_uri("http://sample/2.0");
    let mut reg = PackageRegistry::new();
    reg.register(make_package_ref(pkg));

    assert!(reg.package("sample").is_some());
    assert!(reg.package_by_ns_uri("http://sample/2.0").is_some());
    assert!(reg.package_by_ns_uri("http://other").is_none());
}

#[test]
fn enum_literals_looked_up() {
    let mut e = EEnum::new("AccStatus");
    e.set_serializable(true);
    e.add_literal("ACTIVE", 0);
    e.add_literal("INACTIVE", 1);

    assert_eq!(e.literal_by_name("ACTIVE").unwrap().value(), 0);
    assert_eq!(e.literal_by_value(1).unwrap().name(), "INACTIVE");
    assert!(e.literal_by_name("MISSING").is_none());
}
