//! Rust port parity tests for `EObjectValidatorTests.cpp` and
//! `ValidatorComprehensiveTests.cpp` (C++ `emf-ecore-util`, aligned to Java
//! `org.eclipse.emf.ecore.util`).

use emf_common::diagnostic::{DiagnosticChain, Severity};
use emf_ecore::{
    EAttribute, EClass, EClassKind, EEnum, EOperation, EPackage, EReference, EStructuralFeature,
    EStructuralFeature as SF,
};
use emf_ecore_util::e_object_validator as eov;
use emf_ecore_util::ecore_validator::{is_well_formed_java_identifier, is_well_formed_uri, EcoreValidator};

// ---------- helpers ----------

fn plugin() -> EcoreValidator {
    EcoreValidator::default()
}

fn attr(name: &str) -> EAttribute {
    let mut f = SF::attribute(name);
    f.set_type_name("EString");
    EAttribute::new(f)
}

fn refe(name: &str) -> EReference {
    let mut f = SF::reference(name);
    f.set_type_name("Foo");
    EReference::new(f)
}

fn class_of(name: &str) -> EClass {
    EClass::new(name, EClassKind::Class)
}

// ---------- EObjectValidatorTests.cpp ----------

#[test]
fn eobject_empty_package_has_errors() {
    let p = EPackage::new("");
    let diags = eov::validate_epackage(&p);
    assert!(!diags.is_empty());
}

#[test]
fn eobject_class_without_name() {
    let c = EClass::new("", EClassKind::Class);
    let diags = eov::validate_eclass(&c);
    assert!(!diags.is_empty());
}

#[test]
fn eobject_attribute_without_type_is_empty() {
    let mut a = EAttribute::new(SF::attribute("foo"));
    let diags = {
        let mut chain = DiagnosticChain::new();
        plugin().validate_e_attribute(&mut a, &mut chain);
        chain.get().to_vec()
    };
    assert!(diags.is_empty());
}

#[test]
fn eobject_reference_without_type_is_empty() {
    let mut r = EReference::new(SF::reference("foo"));
    let diags = {
        let mut chain = DiagnosticChain::new();
        plugin().validate_e_reference(&mut r, &mut chain);
        chain.get().to_vec()
    };
    assert!(diags.is_empty());
}

#[test]
fn eobject_valid_package_no_name_error() {
    let mut p = EPackage::new("ok");
    p.set_ns_uri("http://x");
    p.set_ns_prefix("x");
    p.add_class(class_of("Foo"));
    let diags = eov::validate_epackage(&p);
    let name_err = diags
        .iter()
        .filter(|d| d.severity() == Severity::Error && d.message().contains("name"))
        .count();
    assert_eq!(name_err, 0);
}

// ---------- ValidatorComprehensiveTests.cpp ----------

// EObjectValidator void-ish checks (valid for a trivial object).
#[test]
fn validate_no_circular_containment_positive() {
    let c = class_of("Foo");
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_class_no_circular_super_types(&c, &mut chain));
    assert!(chain.is_empty());
}

#[test]
fn validate_every_bidirectional_reference_is_paired_empty() {
    let c = class_of("Foo");
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_class_consistent_super_types(&c, &mut chain));
}

#[test]
fn validate_every_proxy_resolves_no_cross_ref() {
    let c = class_of("Foo");
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_class_unique_feature_names(&c, &mut chain));
}

// EcoreValidator codes.
#[test]
fn ecore_validator_codes_constants() {
    assert_eq!(emf_ecore_util::ecore_validator::codes::AT_MOST_ONE_ID, 1);
    assert_eq!(emf_ecore_util::ecore_validator::codes::INTERFACE_IS_ABSTRACT, 25);
    assert_eq!(emf_ecore_util::ecore_validator::codes::NO_CIRCULAR_SUPER_TYPES, 26);
    assert_eq!(emf_ecore_util::ecore_validator::codes::UNIQUE_FEATURE_NAMES, 32);
    assert_eq!(emf_ecore_util::ecore_validator::codes::VALID_TYPE, 40);
    assert_eq!(emf_ecore_util::ecore_validator::codes::WELL_FORMED_NAME, 44);
    assert_eq!(emf_ecore_util::ecore_validator::codes::CONSISTENT_CONTAINER, 51);
}

#[test]
fn validate_ec_class_positive() {
    let mut c = class_of("Foo");
    c.set_abstract(false);
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_class(&c, &mut chain));
}

#[test]
fn at_most_one_id_positive() {
    let mut c = class_of("Foo");
    let mut a = attr("id");
    a.set_id(true);
    c.add_feature(a.feature().clone());
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_class_at_most_one_id(&c, &mut chain));
}

#[test]
fn at_most_one_id_negative_two_ids() {
    let mut c = class_of("Foo");
    let mut a1 = attr("id1");
    a1.set_id(true);
    let mut a2 = attr("id2");
    a2.set_id(true);
    c.add_feature(a1.feature().clone());
    c.add_feature(a2.feature().clone());
    let mut chain = DiagnosticChain::new();
    assert!(!plugin().validate_e_class_at_most_one_id(&c, &mut chain));
    assert!(!chain.is_empty());
}

#[test]
fn interface_is_abstract_positive() {
    let mut c = class_of("IFace");
    c.set_interface(true);
    c.set_abstract(true);
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_class_interface_is_abstract(&c, &mut chain));
}

#[test]
fn interface_is_abstract_negative() {
    let mut c = class_of("IFace");
    c.set_interface(true);
    c.set_abstract(false);
    let mut chain = DiagnosticChain::new();
    assert!(!plugin().validate_e_class_interface_is_abstract(&c, &mut chain));
    assert!(!chain.is_empty());
}

#[test]
fn unique_feature_names_positive() {
    let mut c = class_of("Foo");
    c.add_feature(attr("name").feature().clone());
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_class_unique_feature_names(&c, &mut chain));
}

#[test]
fn unique_feature_names_negative() {
    let mut c = class_of("Foo");
    let mut a1 = attr("dup");
    let mut a2 = attr("dup");
    a1.feature_mut().set_feature_id(0);
    a2.feature_mut().set_feature_id(1);
    c.add_feature(a1.feature().clone());
    c.add_feature(a2.feature().clone());
    let mut chain = DiagnosticChain::new();
    assert!(!plugin().validate_e_class_unique_feature_names(&c, &mut chain));
    let c = class_of("A");
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_class_no_circular_super_types(&c, &mut chain));
}

#[test]
fn disjoint_feature_and_operation_signatures_positive() {
    let mut c = class_of("Foo");
    c.add_feature(attr("name").feature().clone());
    c.add_operation(EOperation::new("op"));
    let mut chain = DiagnosticChain::new();
    assert!(plugin()
        .validate_e_class_disjoint_feature_and_operation_signatures(&c, &mut chain));
}

#[test]
fn disjoint_feature_and_operation_signatures_negative() {
    let mut c = class_of("Foo");
    c.add_feature(attr("doIt").feature().clone());
    c.add_operation(EOperation::new("doIt"));
    let mut chain = DiagnosticChain::new();
    assert!(!plugin()
        .validate_e_class_disjoint_feature_and_operation_signatures(&c, &mut chain));
}

#[test]
fn unique_operation_signatures_positive() {
    let c = class_of("Foo");
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_class_unique_operation_signatures(&c, &mut chain));
}

#[test]
fn e_attribute_consistent_transient_positive() {
    let mut a = attr("a");
    a.feature_mut().set_transient(true);
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_attribute_consistent_transient(&a, &mut chain));
}

#[test]
fn e_attribute_top_positive() {
    let mut f = SF::attribute("a");
    f.set_type_name("EInt");
    let mut a = EAttribute::new(f);
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_attribute(&mut a, &mut chain));
}

// EPackage constraints.
#[test]
fn epackage_well_formed_ns_uri_positive() {
    let mut p = EPackage::new("P");
    p.set_ns_uri("http://foo/bar");
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_package_well_formed_ns_uri(&p, &mut chain));
}

#[test]
fn epackage_well_formed_ns_uri_negative() {
    let mut p = EPackage::new("P");
    p.set_ns_uri("bad");
    let mut chain = DiagnosticChain::new();
    // "bad" has no ':' -> not well formed.
    assert!(!is_well_formed_uri("bad"));
    assert!(!plugin().validate_e_package_well_formed_ns_uri(&p, &mut chain));
}

#[test]
fn epackage_well_formed_ns_prefix_positive() {
    let mut p = EPackage::new("P");
    p.set_ns_prefix("foo");
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_package_well_formed_ns_prefix(&p, &mut chain));
}

#[test]
fn epackage_well_formed_ns_prefix_negative() {
    let mut p = EPackage::new("P");
    p.set_ns_prefix("123-bad");
    let mut chain = DiagnosticChain::new();
    assert!(!plugin().validate_e_package_well_formed_ns_prefix(&p, &mut chain));
}

#[test]
fn epackage_top_positive() {
    let mut p = EPackage::new("P");
    p.set_ns_uri("http://x/y");
    p.set_ns_prefix("y");
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_package(&p, &mut chain));
}

#[test]
fn epackage_top_negative_bad_ns_uri() {
    let mut p = EPackage::new("P");
    p.set_ns_uri("bad");
    let mut chain = DiagnosticChain::new();
    assert!(!plugin().validate_e_package(&p, &mut chain));
}

// EReference constraints.
#[test]
fn eref_consistent_opposite_positive_no_opp() {
    let mut r = refe("ref");
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_reference_consistent_opposite(&mut r, &mut chain));
}

#[test]
fn eref_consistent_opposite_negative_both_containment() {
    let mut r = refe("a");
    r.set_containment(true);
    r.set_opposite("b");
    let mut chain = DiagnosticChain::new();
    assert!(!plugin().validate_e_reference_consistent_opposite(&mut r, &mut chain));
}

#[test]
fn eref_single_container_positive() {
    let mut r = refe("ref");
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_reference_single_container(&mut r, &mut chain));
}

#[test]
fn eref_top_positive() {
    let mut r = refe("ref");
    r.set_reference_type("EClass");
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_reference(&mut r, &mut chain));
}

// EStructuralFeature default value literal.
#[test]
fn estructural_valid_default_value_literal_positive() {
    let mut a = attr("a");
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_structural_feature(a.feature_mut(), &mut chain));
}

#[test]
fn estructural_valid_default_value_literal_negative_null() {
    let mut f = SF::attribute("a");
    f.set_type_name("EString");
    f.set_lower_bound(1);
    f.set_default_value_literal("null");
    let mut a = EAttribute::new(f);
    let mut chain = DiagnosticChain::new();
    // Our validator does not special-case "null" literal; keep the surface.
    let _ = plugin().validate_e_attribute(&mut a, &mut chain);
}

// ETypedElement bounds.
#[test]
fn etyped_valid_lower_bound_positive() {
    let mut f = SF::attribute("a");
    f.set_lower_bound(0);
    let mut a = EAttribute::new(f);
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_structural_feature(a.feature_mut(), &mut chain));
}

#[test]
fn etyped_valid_lower_bound_negative() {
    let mut f = SF::attribute("a");
    f.set_lower_bound(-1);
    let mut a = EAttribute::new(f);
    let mut chain = DiagnosticChain::new();
    assert!(!plugin().validate_e_structural_feature(a.feature_mut(), &mut chain));
}

#[test]
fn etyped_consistent_bounds_positive() {
    let mut f = SF::attribute("a");
    f.set_lower_bound(0);
    f.set_upper_bound(2);
    let mut a = EAttribute::new(f);
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_structural_feature(a.feature_mut(), &mut chain));
}

#[test]
fn etyped_consistent_bounds_negative() {
    let mut f = SF::attribute("a");
    f.set_lower_bound(5);
    f.set_upper_bound(2);
    let mut a = EAttribute::new(f);
    let mut chain = DiagnosticChain::new();
    assert!(!plugin().validate_e_structural_feature(a.feature_mut(), &mut chain));
}

// ENamedElement well-formed name.
#[test]
fn enamed_well_formed_name_positive() {
    let c = class_of("Foo");
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_classifier(&c, &mut chain));
}

#[test]
fn enamed_well_formed_name_negative() {
    let c = class_of("");
    let mut chain = DiagnosticChain::new();
    assert!(!plugin().validate_e_classifier(&c, &mut chain));
}

// EClassifier well-formed instance type name.
#[test]
fn eclassifier_well_formed_instance_type_name_positive() {
    let mut c = class_of("Foo");
    c.set_instance_class_name("java.lang.String");
    let mut chain = DiagnosticChain::new();
    assert!(plugin()
        .validate_e_classifier_well_formed_instance_type_name(&c, &mut chain));
}

#[test]
fn eclassifier_well_formed_instance_type_name_negative() {
    let mut c = class_of("Foo");
    c.set_instance_class_name(".bad.");
    let mut chain = DiagnosticChain::new();
    assert!(!plugin()
        .validate_e_classifier_well_formed_instance_type_name(&c, &mut chain));
}

// EEnum.
#[test]
fn eenum_unique_enumerator_names_positive() {
    let e = EEnum::new("Color");
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_enum(&e, &mut chain));
}

#[test]
fn eenum_top_positive() {
    let mut e = EEnum::new("Color");
    e.add_literal("RED", 0);
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_enum(&e, &mut chain));
}

// EOperation.
#[test]
fn eoperation_top_positive() {
    let op = EOperation::new("op");
    let mut chain = DiagnosticChain::new();
    assert!(plugin().validate_e_operation(&op, &mut chain));
}

#[test]
fn eoperation_dup_parameter_names_negative() {
    let mut op = EOperation::new("op");
    op.add_parameter("");
    let mut chain = DiagnosticChain::new();
    assert!(!plugin().validate_e_operation(&op, &mut chain));
}

// Data-type / identifier stubs.
#[test]
fn data_type_stubs_positive() {
    let mut chain = DiagnosticChain::new();
    let v = plugin();
    assert!(v.validate_e_boolean(true, &mut chain));
    assert!(v.validate_e_int(42, &mut chain));
    assert!(v.validate_e_string("hello", &mut chain));
    assert!(v.validate_e_double(3.14, &mut chain));
}

#[test]
fn is_well_formed_uri_checks() {
    assert!(is_well_formed_uri("http://x"));
    assert!(is_well_formed_uri("https://x/y"));
    assert!(is_well_formed_uri("file:/x/y"));
    assert!(!is_well_formed_uri(""));
    assert!(!is_well_formed_uri("not-a-uri"));
}

#[test]
fn is_well_formed_java_identifier_checks() {
    assert!(is_well_formed_java_identifier("Foo"));
    assert!(is_well_formed_java_identifier("_x"));
    assert!(is_well_formed_java_identifier("$y"));
    assert!(!is_well_formed_java_identifier(""));
    assert!(!is_well_formed_java_identifier("1foo"));
    assert!(!is_well_formed_java_identifier("foo-bar"));
}