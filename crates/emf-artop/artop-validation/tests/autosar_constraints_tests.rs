//! Integration tests porting `AutosarConstraintsTests.cpp` (13 scenarios) onto
//! the Rust `artop-validation` crate.
//!
//! The dynamic model (Container -> elements:Element[], others:Other[];
//! Element has shortName/uuid/category(lowerBound=1) + non-containment `ref`)
//! is built with `DynamicEObject` + `adopt_many`, matching the C++ XMI-described
//! test Ecore. Proxies are created with `e_set_proxy_uri` (like the C++
//! `EObjectImpl::eSetProxyURI`).

use std::cell::RefCell;
use std::rc::Rc;

use artop_validation::{
    register_autosar_constraints, validate_named_single, validate_named_tree,
    validate_uuid_uniqueness,
};
use emf_common::diagnostic::Diagnostic;
use emf_common::eobject::EObject;
use emf_common::uri::Uri;
use emf_common::value::Val;
use emf_ecore::{
    adopt_many, node_to_object, DynNode, DynamicEObject, EClass, EClassKind, EStructuralFeature,
};
use emf_validation::e_validator::EValidator;
use emf_validation::validation_service::ValidationService;

/// Test metamodel handle.
struct Model {
    container: EClass,
    element: EClass,
    other: EClass,
}

/// Build the dynamic metamodel (mirrors `kAutosarTestEcore`).
fn build_model() -> Model {
    let mut container = EClass::new("Container", EClassKind::Class);
    let mut elements = EStructuralFeature::reference_many("elements");
    elements.set_containment(true);
    elements.set_type_name("Element");
    container.add_feature(elements);
    let mut others = EStructuralFeature::reference_many("others");
    others.set_containment(true);
    others.set_type_name("Other");
    container.add_feature(others);

    let mut element = EClass::new("Element", EClassKind::Class);
    let mut sn = EStructuralFeature::attribute("shortName");
    sn.set_type_name("EString");
    element.add_feature(sn);
    let mut uuid = EStructuralFeature::attribute("uuid");
    uuid.set_type_name("EString");
    element.add_feature(uuid);
    let mut cat = EStructuralFeature::attribute("category");
    cat.set_type_name("EString");
    cat.set_lower_bound(1);
    element.add_feature(cat);
    let mut rf = EStructuralFeature::reference("ref"); // non-containment
    rf.set_type_name("Element");
    element.add_feature(rf);

    let mut other = EClass::new("Other", EClassKind::Class);
    let mut osn = EStructuralFeature::attribute("shortName");
    osn.set_type_name("EString");
    other.add_feature(osn);

    Model {
        container,
        element,
        other,
    }
}

fn make_node(_m: &Model, cls: &EClass) -> DynNode {
    Rc::new(RefCell::new(DynamicEObject::new(cls.clone())))
}

fn make_element(m: &Model, sn: &str, uuid: &str, category: &str) -> DynNode {
    let e = make_node(m, &m.element);
    {
        let mut o = e.borrow_mut();
        o.e_set("shortName", Val::string(sn));
        o.e_set("uuid", Val::string(uuid));
        o.e_set("category", Val::string(category));
    }
    e
}

fn make_other(m: &Model, sn: &str) -> DynNode {
    let o = make_node(m, &m.other);
    o.borrow_mut().e_set("shortName", Val::string(sn));
    o
}

fn has_diag_with(diags: &[Diagnostic], sub: &str) -> bool {
    diags.iter().any(|d| d.source().contains(sub))
}

fn validate_all(_m: &Model, root: &DynNode) -> Vec<Diagnostic> {
    let mut validator = EValidator::new();
    register_autosar_constraints(&mut validator);
    // Named-source validation: each diagnostic's `source` is the violating
    // constraint name (AutosarShortNameNonEmpty, ...), matching C++.
    validate_named_tree(&validator, None, &*root.borrow())
}

// ===== 测试 1：valid 模型 → 无 AUTOSAR diagnostic =====
#[test]
fn valid_model_no_autosar_diagnostics() {
    let m = build_model();
    let container = make_node(&m, &m.container);
    let e0 = make_element(&m, "ElemA", "uuid-A", "default");
    let e1 = make_element(&m, "ElemB", "uuid-B", "default");
    adopt_many(&container, "elements", &e0);
    adopt_many(&container, "elements", &e1);

    let diags = validate_all(&m, &container);
    assert!(!has_diag_with(&diags, "Autosar"));
}

// ===== 测试 2：空 shortName → AutosarShortNameNonEmpty =====
#[test]
fn empty_short_name_produces_diagnostic() {
    let m = build_model();
    let container = make_node(&m, &m.container);
    let e = make_element(&m, "", "uuid-A", "default");
    adopt_many(&container, "elements", &e);

    let diags = validate_all(&m, &container);
    assert!(has_diag_with(&diags, "AutosarShortNameNonEmpty"));
}

// ===== 测试 3：同父同类型兄弟 shortName 重复 → AutosarShortNameUniqueInParent =====
#[test]
fn duplicate_short_name_same_type_produces_diagnostic() {
    let m = build_model();
    let container = make_node(&m, &m.container);
    let e0 = make_element(&m, "Dup", "uuid-A", "default");
    let e1 = make_element(&m, "Dup", "uuid-B", "default");
    adopt_many(&container, "elements", &e0);
    adopt_many(&container, "elements", &e1);

    let diags = validate_all(&m, &container);
    assert!(has_diag_with(&diags, "AutosarShortNameUniqueInParent"));
}

// ===== 测试 4：异型兄弟 shortName 相同 → 无唯一性 diagnostic =====
#[test]
fn duplicate_short_name_different_type_no_uniqueness_diagnostic() {
    let m = build_model();
    let container = make_node(&m, &m.container);
    let e = make_element(&m, "Shared", "uuid-A", "default");
    let o = make_other(&m, "Shared");
    adopt_many(&container, "elements", &e);
    adopt_many(&container, "others", &o);

    let diags = validate_all(&m, &container);
    assert!(!has_diag_with(&diags, "AutosarShortNameUniqueInParent"));
}

// ===== 测试 5：空 uuid → AutosarUuidNonEmpty =====
#[test]
fn empty_uuid_produces_diagnostic() {
    let m = build_model();
    let container = make_node(&m, &m.container);
    let e = make_element(&m, "ElemA", "", "default");
    adopt_many(&container, "elements", &e);

    let diags = validate_all(&m, &container);
    assert!(has_diag_with(&diags, "AutosarUuidNonEmpty"));
}

// ===== 测试 6：空 category（lowerBound=1）→ AutosarCategoryRequired =====
#[test]
fn empty_category_produces_diagnostic() {
    let m = build_model();
    let container = make_node(&m, &m.container);
    let e = make_element(&m, "ElemA", "uuid-A", "");
    adopt_many(&container, "elements", &e);

    let diags = validate_all(&m, &container);
    assert!(has_diag_with(&diags, "AutosarCategoryRequired"));
}

// ===== 测试 7：未解析 proxy 引用 → AutosarNoUnresolvedProxy =====
#[test]
fn unresolved_proxy_produces_diagnostic() {
    let m = build_model();
    let container = make_node(&m, &m.container);
    let e = make_element(&m, "ElemA", "uuid-A", "default");
    let proxy = make_element(&m, "Proxy", "uuid-P", "default");
    proxy
        .borrow_mut()
        .e_set_proxy_uri(Some(Uri::parse("file:/nonexistent.arxml#//Unresolved")));
    assert!(proxy.borrow().e_is_proxy());
    e.borrow_mut()
        .e_set("ref", Val::object(node_to_object(&proxy)));
    adopt_many(&container, "elements", &e);

    let diags = validate_all(&m, &container);
    assert!(has_diag_with(&diags, "AutosarNoUnresolvedProxy"));
}

// ===== 测试 8：LIVE 模式——变更后重校验触发 AutosarShortNameNonEmpty =====
//
// Trade-off vs C++ `ValidationLiveAdapter.attach`: here the Rust `LiveValidator`
// is a thin wrapper and `DynamicEObject` emits no change notifications, so we
// drive the dual-mode constraints through the registered `ValidationService`
// and re-validate after each mutation (`validate_named_single`), asserting that
// mutating `shortName` to an empty value yields AutosarShortNameNonEmpty.
#[test]
fn live_empty_short_name_triggers_diagnostic() {
    let m = build_model();
    let container = make_node(&m, &m.container);
    let e = make_element(&m, "ElemA", "uuid-A", "default");
    adopt_many(&container, "elements", &e);

    // Dual-mode constraints are registered into a ValidationService; because
    // the Rust `DynamicEObject` emits no change notifications (the `LiveValidator`
    // here is a thin wrapper), we re-drive validation after each mutation and
    // assert on the named-source diagnostic. See the note at the top of this
    // module about the alignment trade-off vs C++ `ValidationLiveAdapter.attach`.
    let mut svc = ValidationService::new();
    register_autosar_constraints(svc.validator());

    e.borrow_mut().e_set("shortName", Val::string("StillValid"));
    let diags = validate_named_single(svc.validator_ref(), None, &*e.borrow());
    assert!(!has_diag_with(&diags, "AutosarShortNameNonEmpty"));

    e.borrow_mut().e_set("shortName", Val::string(""));
    let diags = validate_named_single(svc.validator_ref(), None, &*e.borrow());
    assert!(has_diag_with(&diags, "AutosarShortNameNonEmpty"));
}

// ===== 测试 9：幂等——重复注册替换旧约束，约束总数不翻倍 =====
#[test]
fn register_twice_is_idempotent() {
    let mut validator = EValidator::new();
    register_autosar_constraints(&mut validator);
    let n1 = validator.constraints().len();
    register_autosar_constraints(&mut validator);
    let n2 = validator.constraints().len();
    assert_eq!(n1, n2);
}

// ===== 测试 10：UUID 全局唯一——不同 uuid → 无 diagnostic =====
#[test]
fn unique_uuids_no_globally_unique_diagnostic() {
    let m = build_model();
    let container = make_node(&m, &m.container);
    let e0 = make_element(&m, "ElemA", "uuid-A", "default");
    let e1 = make_element(&m, "ElemB", "uuid-B", "default");
    let e2 = make_element(&m, "ElemC", "uuid-C", "default");
    adopt_many(&container, "elements", &e0);
    adopt_many(&container, "elements", &e1);
    adopt_many(&container, "elements", &e2);

    let diags = validate_all(&m, &container);
    assert!(!has_diag_with(&diags, "AutosarUuidGloballyUnique"));
}

// ===== 测试 11：UUID 全局唯一——重复 uuid → AutosarUuidGloballyUnique =====
#[test]
fn duplicate_uuid_produces_globally_unique_diagnostic() {
    let m = build_model();
    let container = make_node(&m, &m.container);
    let e0 = make_element(&m, "ElemA", "same-uuid", "default");
    let e1 = make_element(&m, "ElemB", "same-uuid", "default");
    adopt_many(&container, "elements", &e0);
    adopt_many(&container, "elements", &e1);

    let diags = validate_all(&m, &container);
    assert!(has_diag_with(&diags, "AutosarUuidGloballyUnique"));
}

// ===== 测试 12：UUID 全局唯一——空 uuid 独立校验也报告 =====
#[test]
fn empty_uuid_produces_globally_unique_diagnostic() {
    let m = build_model();
    let container = make_node(&m, &m.container);
    let e = make_element(&m, "ElemA", "", "default");
    adopt_many(&container, "elements", &e);

    let diags = validate_uuid_uniqueness(&*container.borrow());
    assert!(has_diag_with(&diags, "AutosarUuidGloballyUnique"));
}

// ===== 测试 13：UUID 全局唯一——深层嵌套重复 uuid 也能检测 =====
#[test]
fn deep_nested_duplicate_uuid_produces_diagnostic() {
    let m = build_model();
    let outer = make_node(&m, &m.container);
    let inner = make_node(&m, &m.container);
    let e0 = make_element(&m, "ElemA", "deep-uuid", "default");
    let e1 = make_element(&m, "ElemB", "deep-uuid", "default");
    adopt_many(&outer, "elements", &e0);
    adopt_many(&outer, "others", &inner);
    adopt_many(&inner, "elements", &e1);

    let diags = validate_all(&m, &outer);
    assert!(has_diag_with(&diags, "AutosarUuidGloballyUnique"));
}