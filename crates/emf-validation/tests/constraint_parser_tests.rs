//! Rust port parity tests for `ConstraintParserTests.cpp` (C++ `emf-validation`,
//! aligned to the Eclipse OCL / EMF Validation OCL evaluator).
//!
//! The metamodel (Container -> elements:Element[] / single:Element,
//! Element: shortName / count / ref / children) is built by hand with
//! `DynamicEObject` + `EClass` descriptors (no XMI dependency).

use emf_common::eobject::EObject;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::{
    node_to_object, DynamicEObject, EClass, EClassKind, EStructuralFeature, EPackage,
    PackageRegistry, adopt_many, adopt_single, make_package_ref,
};
use emf_validation::constraint::Severity;
use emf_validation::constraint_parser::{compile, compile_value, parse};
use std::cell::RefCell;
use std::rc::Rc;

type DynNode = Rc<RefCell<DynamicEObject>>;

/// Build the test metamodel and return (Container class, Element class).
fn meta() -> (EClass, EClass) {
    let mut pkg = EPackage::new("oclp");
    pkg.set_ns_prefix("oclp");
    pkg.set_ns_uri("http://example.com/oclp/1.0");

    let mut container = EClass::new("Container", EClassKind::Class);
    let mut elements = EStructuralFeature::reference_many("elements");
    elements.set_type_name("Element");
    elements.set_containment(true);
    container.add_feature(elements);
    let mut single = EStructuralFeature::reference("single");
    single.set_type_name("Element");
    single.set_containment(true);
    container.add_feature(single);
    pkg.add_class(container);

    let mut element = EClass::new("Element", EClassKind::Class);
    let mut short_name = EStructuralFeature::attribute("shortName");
    short_name.set_type_name("EString");
    element.add_feature(short_name);
    let mut count = EStructuralFeature::attribute("count");
    count.set_type_name("EInt");
    element.add_feature(count);
    let mut ref_ = EStructuralFeature::reference("ref");
    ref_.set_type_name("Element");
    element.add_feature(ref_);
    let mut children = EStructuralFeature::reference_many("children");
    children.set_type_name("Element");
    children.set_containment(true);
    element.add_feature(children);
    pkg.add_class(element);

    let mut r = PackageRegistry::new();
    r.register(make_package_ref(pkg));
    let container_cls = r.find_class("Container").unwrap();
    let element_cls = r.find_class("Element").unwrap();
    (container_cls, element_cls)
}

fn new_container(cls: &EClass) -> DynNode {
    Rc::new(RefCell::new(DynamicEObject::new(cls.clone())))
}

fn make_element(cls: &EClass, sn: &str, count: i32) -> DynNode {
    let e = Rc::new(RefCell::new(DynamicEObject::new(cls.clone())));
    e.borrow_mut().e_set("shortName", Val::string(sn));
    e.borrow_mut().e_set("count", Val::Int(count as i64));
    e
}

fn make_plain_element(cls: &EClass, sn: &str) -> DynNode {
    let e = Rc::new(RefCell::new(DynamicEObject::new(cls.clone())));
    e.borrow_mut().e_set("shortName", Val::string(sn));
    e
}

fn set_elements(container: &DynNode, kids: Vec<DynNode>) {
    for k in kids {
        adopt_many(container, "elements", &k);
    }
}

fn set_children(element: &DynNode, kids: Vec<DynNode>) {
    for k in kids {
        adopt_many(element, "children", &k);
    }
}

fn set_single(container: &DynNode, child: &DynNode) {
    adopt_single(container, "single", child);
}

fn set_ref(element: &DynNode, target: &DynNode) {
    element
        .borrow_mut()
        .e_set("ref", Val::Object(node_to_object(target)));
}

/// Convenience: compile and evaluate an expression over a target.
fn ev(expr: &str, node: &DynNode) -> bool {
    let obj: ObjectRef = node_to_object(node);
    let e = compile(expr);
    let x = e(&*obj.borrow());
    x
}

fn to_dyn(node: &DynNode) -> ObjectRef {
    node_to_object(node)
}

// ===== backward compat: original tests =====

#[test]
fn constraint_parser_compile_returns_evaluator() {
    // C++ passes nullptr target -> true. Rust has no null target; the evaluator
    // is still produced and callable. An unset "attr" reads null, so
    // `attr != null` evaluates to false on a real object.
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(!ev("attr != null", &c));
}

#[test]
fn constraint_parser_parse_returns_constraint() {
    let c = parse("src", "MyConstraint", "x > 0", Severity::Warning);
    assert_eq!(c.name(), "MyConstraint");
}

// ===== implies =====

#[test]
fn ocl_implies_true_implies_false_is_false() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(!ev("true implies false", &c));
}

#[test]
fn ocl_implies_false_implies_false_is_true() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("false implies false", &c));
}

#[test]
fn ocl_implies_true_implies_true_is_true() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("true implies true", &c));
}

#[test]
fn ocl_implies_right_associative() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("false implies false implies false", &c));
}

#[test]
fn ocl_implies_with_comparison() {
    let (_, e_cls) = meta();
    let e = make_element(&e_cls, "x", 5);
    // ref not set -> null; true implies false -> false
    assert!(!ev("self.shortName <> '' implies self.ref <> null", &e));

    let target = make_plain_element(&e_cls, "t");
    set_ref(&e, &target);
    assert!(ev("self.shortName <> '' implies self.ref <> null", &e));

    let e2 = make_element(&e_cls, "", 0);
    assert!(ev("self.shortName <> '' implies self.ref <> null", &e2));
}

// ===== forAll =====

#[test]
fn ocl_forall_all_satisfy_is_true() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(ev("self.elements->forAll(x | x.shortName <> '')", &c));
}

#[test]
fn ocl_forall_one_fails_is_false() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, ""),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(!ev("self.elements->forAll(x | x.shortName <> '')", &c));
}

#[test]
fn ocl_forall_empty_collection_is_true() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    let _ = &e_cls;
    assert!(ev("self.elements->forAll(x | x.shortName <> '')", &c));
}

#[test]
fn ocl_forall_numeric_condition() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![make_element(&e_cls, "a", 3), make_element(&e_cls, "b", 5)],
    );
    assert!(ev("self.elements->forAll(x | x.count > 0)", &c));
    assert!(!ev("self.elements->forAll(x | x.count > 4)", &c));
}

#[test]
fn ocl_forall_with_implies_in_body() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![make_element(&e_cls, "a", 3), make_element(&e_cls, "", 0)],
    );
    assert!(ev(
        "self.elements->forAll(x | x.count > 0 implies x.shortName <> '')",
        &c
    ));
}

// ===== exists =====

#[test]
fn ocl_exists_one_satisfies_is_true() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "target"),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(ev(
        "self.elements->exists(x | x.shortName = 'target')",
        &c
    ));
}

#[test]
fn ocl_exists_none_satisfy_is_false() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
        ],
    );
    assert!(!ev(
        "self.elements->exists(x | x.shortName = 'target')",
        &c
    ));
}

#[test]
fn ocl_exists_empty_collection_is_false() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    let _ = &e_cls;
    assert!(!ev(
        "self.elements->exists(x | x.shortName = 'target')",
        &c
    ));
}

// ===== single reference treated as singleton =====

#[test]
fn ocl_single_ref_forall_treated_as_singleton() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    let e = make_plain_element(&e_cls, "only");
    set_single(&c, &e);
    assert!(ev("self.single->forAll(x | x.shortName = 'only')", &c));
}

#[test]
fn ocl_single_ref_null_forall_is_true() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("self.single->forAll(x | x.shortName = 'only')", &c));
}

#[test]
fn ocl_single_ref_null_exists_is_false() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(!ev("self.single->exists(x | x.shortName = 'only')", &c));
}

// ===== logical operators =====

#[test]
fn ocl_and_both_true_is_true() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("true and true", &c));
}

#[test]
fn ocl_and_one_false_is_false() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(!ev("true and false", &c));
}

#[test]
fn ocl_or_both_false_is_false() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(!ev("false or false", &c));
}

#[test]
fn ocl_or_one_true_is_true() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("false or true", &c));
}

#[test]
fn ocl_not_true_is_false() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(!ev("not true", &c));
}

#[test]
fn ocl_not_not_false_is_true() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("not not true", &c));
}

#[test]
fn ocl_and_or_precedence() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(!ev("false or true and false", &c));
}

#[test]
fn ocl_paren_grouping() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(!ev("(false or true) and false", &c));
}

// ===== OCL equality =====

#[test]
fn ocl_ocl_equality_op_equals() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(&c, vec![make_plain_element(&e_cls, "foo")]);
    assert!(ev("self.elements->forAll(x | x.shortName = 'foo')", &c));
}

#[test]
fn ocl_ocl_inequality_op_diamond() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(&c, vec![make_plain_element(&e_cls, "foo")]);
    assert!(!ev("self.elements->exists(x | x.shortName <> 'foo')", &c));
}

// ===== collection size / isEmpty / notEmpty =====

#[test]
fn ocl_collection_size() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(ev("self.elements->size() = 3", &c));
    assert!(ev("self.elements->size() > 2", &c));
    assert!(!ev("self.elements->size() = 2", &c));
}

#[test]
fn ocl_collection_is_empty() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    assert!(ev("self.elements->isEmpty()", &c));
    set_elements(&c, vec![make_plain_element(&e_cls, "a")]);
    assert!(!ev("self.elements->isEmpty()", &c));
}

#[test]
fn ocl_collection_not_empty() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    assert!(!ev("self.elements->notEmpty()", &c));
    set_elements(&c, vec![make_plain_element(&e_cls, "a")]);
    assert!(ev("self.elements->notEmpty()", &c));
}

// ===== path navigation =====

#[test]
fn ocl_path_navigation_self_attr() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(&c, vec![make_plain_element(&e_cls, "hello")]);
    assert!(ev(
        "self.elements->forAll(x | x.shortName <> '')",
        &c
    ));
}

#[test]
fn ocl_path_navigation_implicit_self() {
    let (_, e_cls) = meta();
    let e = make_plain_element(&e_cls, "world");
    assert!(ev("shortName = 'world'", &e));
    assert!(!ev("shortName = 'other'", &e));
}

#[test]
fn ocl_path_navigation_deep_path() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    let e0 = make_plain_element(&e_cls, "first");
    let e1 = make_plain_element(&e_cls, "second");
    set_ref(&e0, &e1);
    set_elements(&c, vec![e0]);
    assert!(ev(
        "self.elements->exists(x | x.ref.shortName = 'second')",
        &c
    ));
    assert!(!ev(
        "self.elements->exists(x | x.ref.shortName = 'first')",
        &c
    ));
}

#[test]
fn ocl_dot_size_on_collection() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
        ],
    );
    assert!(ev("self.elements.size() = 2", &c));
}

// ===== if-then-else =====

#[test]
fn ocl_if_then_else_then_branch() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("if true then true else false endif", &c));
}

#[test]
fn ocl_if_then_else_else_branch() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(!ev("if false then true else false endif", &c));
}

#[test]
fn ocl_if_then_else_with_condition() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(&c, vec![make_plain_element(&e_cls, "a")]);
    assert!(ev(
        "if self.elements->notEmpty() then true else false endif",
        &c
    ));
}

// ===== null checks =====

#[test]
fn ocl_null_check_ref_not_null() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    let e = make_plain_element(&e_cls, "a");
    let target = make_plain_element(&e_cls, "b");
    set_ref(&e, &target);
    set_elements(&c, vec![e]);
    assert!(ev("self.elements->forAll(x | x.ref <> null)", &c));
}

#[test]
fn ocl_null_check_ref_is_null() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    let e = make_plain_element(&e_cls, "a");
    set_elements(&c, vec![e]);
    assert!(ev("self.elements->forAll(x | x.ref = null)", &c));
    assert!(!ev("self.elements->forAll(x | x.ref <> null)", &c));
}

// ===== tolerant fallback =====

#[test]
fn ocl_fallback_invalid_expression_returns_true() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("this is not valid ocl @#$", &c));
}

#[test]
fn ocl_fallback_empty_expression_returns_true() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("", &c));
}

// ===== value constraint argument =====

#[test]
fn ocl_value_param_numeric_compare() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    let obj = to_dyn(&c);
    let eval = compile_value("value > 5");
    assert!(eval(&*obj.borrow(), Some(Val::Int(10))));
    assert!(!eval(&*obj.borrow(), Some(Val::Int(3))));
}

// ===== combined complex expressions =====

#[test]
fn ocl_complex_forall_with_or_and_implies() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![make_element(&e_cls, "a", 3), make_element(&e_cls, "zero", 0)],
    );
    assert!(ev(
        "self.elements->forAll(x | x.count > 0 or x.shortName = 'zero')",
        &c
    ));
}

#[test]
fn ocl_complex_exists_with_not() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![make_element(&e_cls, "a", 1), make_element(&e_cls, "b", 5)],
    );
    assert!(ev(
        "self.elements->exists(x | not x.shortName = '' and x.count > 2)",
        &c
    ));
    assert!(!ev(
        "self.elements->exists(x | x.shortName = '' and x.count > 2)",
        &c
    ));
}

// ===== let expressions =====

#[test]
fn ocl_let_simple_binding_is_true() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("let x = 5 in x > 3", &c));
    assert!(!ev("let x = 5 in x > 10", &c));
}

#[test]
fn ocl_let_with_optional_type_annotation() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("let x : Integer = 10 in x > 5", &c));
}

#[test]
fn ocl_let_nested_bindings() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("let a = 1 in let b = 2 in a + b > 2", &c));
    assert!(!ev("let a = 1 in let b = 2 in a + b > 5", &c));
}

#[test]
fn ocl_let_binds_attribute() {
    let (_, e_cls) = meta();
    let e = make_plain_element(&e_cls, "hello");
    assert!(ev("let n = self.shortName in n.size() > 0", &e));
    assert!(ev("let n = self.shortName in n = 'hello'", &e));
}

#[test]
fn ocl_let_scope_is_removed_after_body() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev(
        "let x = 5 in (let y = 10 in y > 5) and x = 5",
        &c
    ));
}

// ===== collection comprehensions =====

#[test]
fn ocl_collect_returns_element_collection() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(ev("self.elements->collect(c | c)->size() > 0", &c));
    assert!(!ev("self.elements->collect(c | c)->isEmpty()", &c));
}

#[test]
fn ocl_collect_pulls_attribute() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(ev(
        "self.elements->collect(c | c.shortName)->size() = 3",
        &c
    ));
}

#[test]
fn ocl_select_filters_subset() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(ev(
        "self.elements->select(c | c.shortName = 'a')->size() = 1",
        &c
    ));
    assert!(ev(
        "self.elements->select(c | c.shortName = 'z')->isEmpty()",
        &c
    ));
}

#[test]
fn ocl_reject_excludes_matching() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(ev(
        "self.elements->reject(c | c.shortName = 'a')->size() = 2",
        &c
    ));
}

#[test]
fn ocl_any_returns_first_matching() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(ev(
        "self.elements->any(c | c.shortName = 'a').shortName = 'a'",
        &c
    ));
    assert!(ev(
        "self.elements->any(c | c.shortName = 'z').oclIsUndefined()",
        &c
    ));
}

#[test]
fn ocl_iterate_counts_elements() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(ev(
        "self.elements->iterate(c; sum : Integer = 0 | sum + 1) > 0",
        &c
    ));
    assert!(ev(
        "self.elements->iterate(c; sum : Integer = 0 | sum + 1) = 3",
        &c
    ));
}

#[test]
fn ocl_iterate_sums_counts() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![make_element(&e_cls, "a", 3), make_element(&e_cls, "b", 5)],
    );
    assert!(ev(
        "self.elements->iterate(c; sum : Integer = 0 | sum + c.count) = 8",
        &c
    ));
}

// ===== String library =====

#[test]
fn ocl_string_to_upper() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("'hello'.toUpper() = 'HELLO'", &c));
}

#[test]
fn ocl_string_to_lower() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("'HELLO'.toLower() = 'hello'", &c));
}

#[test]
fn ocl_string_concat() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("'ab'.concat('cd') = 'abcd'", &c));
}

#[test]
fn ocl_string_substring() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("'hello'.substring(2, 4) = 'ell'", &c));
    assert!(ev("'hello'.substring(1, 1) = 'h'", &c));
}

#[test]
fn ocl_string_starts_with() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("'hello'.startsWith('he')", &c));
    assert!(!ev("'hello'.startsWith('lo')", &c));
}

#[test]
fn ocl_string_ends_with() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("'hello'.endsWith('lo')", &c));
    assert!(!ev("'hello'.endsWith('he')", &c));
}

#[test]
fn ocl_string_index_of() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("'hello'.indexOf('l') = 3", &c));
    assert!(ev("'hello'.indexOf('z') = 0", &c));
}

#[test]
fn ocl_string_trim() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("'  hi  '.trim() = 'hi'", &c));
}

#[test]
fn ocl_string_length() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("'hello'.length() = 5", &c));
}

#[test]
fn ocl_string_plus_operator() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("'ab' + 'cd' = 'abcd'", &c));
}

// ===== Integer / Real library =====

#[test]
fn ocl_integer_abs() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("(-5).abs() = 5", &c));
    assert!(ev("5.abs() = 5", &c));
}

#[test]
fn ocl_integer_max() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("3.max(5) = 5", &c));
    assert!(ev("3.max(2) = 3", &c));
}

#[test]
fn ocl_integer_min() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("3.min(5) = 3", &c));
    assert!(ev("3.min(2) = 2", &c));
}

#[test]
fn ocl_integer_mod() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("7.mod(3) = 1", &c));
}

#[test]
fn ocl_integer_div() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("7.div(3) = 2", &c));
}

#[test]
fn ocl_integer_floor() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("(3.7).floor() = 3", &c));
}

#[test]
fn ocl_integer_to_string() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("(42).toString().size() > 0", &c));
}

// ===== general object operations =====

#[test]
fn ocl_ocl_is_undefined_missing_attr_is_true() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    assert!(ev("self.missing.oclIsUndefined()", &c));
    let e = make_plain_element(&e_cls, "x");
    assert!(!ev("self.shortName.oclIsUndefined()", &e));
}

#[test]
fn ocl_ocl_is_invalid_always_false() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(!ev("self.oclIsInvalid()", &c));
    assert!(ev("not self.oclIsInvalid()", &c));
}

#[test]
fn ocl_ocl_is_kind_of_self_class() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    assert!(ev("self.oclIsKindOf(Container)", &c));
    assert!(!ev("self.oclIsKindOf(Element)", &c));
    let e = make_plain_element(&e_cls, "x");
    assert!(ev("self.oclIsKindOf(Element)", &e));
    assert!(!ev("self.oclIsKindOf(Container)", &e));
}

#[test]
fn ocl_ocl_is_type_of_exact_class() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    assert!(ev("self.oclIsTypeOf(Container)", &c));
    assert!(!ev("self.oclIsTypeOf(Element)", &c));
    let _ = &e_cls;
}

#[test]
fn ocl_as_sequence_on_single_ref() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    let e = make_plain_element(&e_cls, "only");
    set_single(&c, &e);
    assert!(ev("self.single.asSequence()->size() = 1", &c));
}

// ===== arithmetic =====

#[test]
fn ocl_arithmetic_precedence() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("2 + 3 * 4 = 14", &c));
    assert!(ev("(2 + 3) * 4 = 20", &c));
}

#[test]
fn ocl_arithmetic_subtraction() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("10 - 3 - 2 = 5", &c));
}

#[test]
fn ocl_arithmetic_division() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("8 / 2 = 4", &c));
}

// ===== nested collection comprehensions =====

#[test]
fn ocl_nested_collect_flattens_inner_collect() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    let e0 = make_plain_element(&e_cls, "e0");
    let e1 = make_plain_element(&e_cls, "e1");
    set_children(
        &e0,
        vec![make_plain_element(&e_cls, "a"), make_plain_element(&e_cls, "b")],
    );
    set_children(&e1, vec![make_plain_element(&e_cls, "d")]);
    set_elements(&c, vec![e0, e1]);
    assert!(ev(
        "self.elements->collect(x | x.children->collect(y | y.shortName))->size() = 3",
        &c
    ));
    assert!(ev(
        "self.elements->collect(x | x.children->collect(y | y.shortName))->includes('a')",
        &c
    ));
    assert!(ev(
        "self.elements->collect(x | x.children->collect(y | y.shortName))->excludes('z')",
        &c
    ));
}

#[test]
fn ocl_nested_select_with_exists_in_body() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    let e0 = make_plain_element(&e_cls, "e0");
    let e1 = make_plain_element(&e_cls, "e1");
    set_children(
        &e0,
        vec![make_plain_element(&e_cls, "a"), make_plain_element(&e_cls, "b")],
    );
    set_children(&e1, vec![make_plain_element(&e_cls, "d")]);
    set_elements(&c, vec![e0, e1]);
    assert!(ev(
        "self.elements->select(x | x.children->exists(y | y.shortName = 'a'))->size() = 1",
        &c
    ));
    assert!(ev(
        "self.elements->select(x | x.children->exists(y | y.shortName = 'z'))->isEmpty()",
        &c
    ));
}

#[test]
fn ocl_nested_forall_deep_quantifier() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    let e0 = make_plain_element(&e_cls, "e0");
    let e1 = make_plain_element(&e_cls, "e1");
    set_children(
        &e0,
        vec![make_plain_element(&e_cls, "a"), make_plain_element(&e_cls, "b")],
    );
    set_children(&e1, vec![make_plain_element(&e_cls, "d")]);
    set_elements(&c, vec![e0, e1]);
    assert!(ev(
        "self.elements->forAll(x | x.children->forAll(y | y.shortName.size() > 0))",
        &c
    ));

    let c2 = new_container(&c_cls);
    let p = make_plain_element(&e_cls, "p");
    set_children(
        &p,
        vec![make_plain_element(&e_cls, ""), make_plain_element(&e_cls, "ok")],
    );
    set_elements(&c2, vec![p]);
    assert!(!ev(
        "self.elements->forAll(x | x.children->forAll(y | y.shortName.size() > 0))",
        &c2
    ));
}

// ===== sortedBy / first / last / at / indexOf / count =====

#[test]
fn ocl_sorted_by_string_key_first_last() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "c"),
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
        ],
    );
    assert!(ev(
        "self.elements->sortedBy(x | x.shortName)->first().shortName = 'a'",
        &c
    ));
    assert!(ev(
        "self.elements->sortedBy(x | x.shortName)->last().shortName = 'c'",
        &c
    ));
}

#[test]
fn ocl_sorted_by_numeric_key() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_element(&e_cls, "x", 3),
            make_element(&e_cls, "y", 1),
            make_element(&e_cls, "z", 2),
        ],
    );
    assert!(ev(
        "self.elements->sortedBy(x | x.count)->first().count = 1",
        &c
    ));
    assert!(ev(
        "self.elements->sortedBy(x | x.count)->last().count = 3",
        &c
    ));
}

#[test]
fn ocl_first_last_insertion_order() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "first"),
            make_plain_element(&e_cls, "mid"),
            make_plain_element(&e_cls, "last"),
        ],
    );
    assert!(ev("self.elements->first().shortName = 'first'", &c));
    assert!(ev("self.elements->last().shortName = 'last'", &c));
    let empty = new_container(&c_cls);
    assert!(ev("self.elements->first().oclIsUndefined()", &empty));
    assert!(ev("self.elements->last().oclIsUndefined()", &empty));
}

#[test]
fn ocl_at_one_based_index() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "first"),
            make_plain_element(&e_cls, "second"),
            make_plain_element(&e_cls, "third"),
        ],
    );
    assert!(ev("self.elements->at(1).shortName = 'first'", &c));
    assert!(ev("self.elements->at(2).shortName = 'second'", &c));
    assert!(ev("self.elements->at(3).shortName = 'third'", &c));
    assert!(ev("self.elements->at(0).oclIsUndefined()", &c));
    assert!(ev("self.elements->at(99).oclIsUndefined()", &c));
}

#[test]
fn ocl_index_of_one_based() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(ev("self.elements->indexOf(self.elements->first()) = 1", &c));
    assert!(ev("self.elements->indexOf(self.elements->last()) = 3", &c));
    assert!(ev("self.elements->indexOf(self.elements->at(2)) = 2", &c));
    assert!(ev("self.elements->indexOf(null) = 0", &c));
}

#[test]
fn ocl_count_occurrences() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "a"),
        ],
    );
    assert!(ev(
        "self.elements->collect(x | x.shortName)->count('a') = 2",
        &c
    ));
    assert!(ev(
        "self.elements->collect(x | x.shortName)->count('b') = 1",
        &c
    ));
    assert!(ev(
        "self.elements->collect(x | x.shortName)->count('z') = 0",
        &c
    ));
    assert!(ev(
        "self.elements->count(self.elements->first()) = 1",
        &c
    ));
}

#[test]
fn ocl_includes_excludes() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(ev(
        "self.elements->collect(x | x.shortName)->includes('a')",
        &c
    ));
    assert!(!ev(
        "self.elements->collect(x | x.shortName)->includes('z')",
        &c
    ));
    assert!(ev(
        "self.elements->collect(x | x.shortName)->excludes('z')",
        &c
    ));
    assert!(!ev(
        "self.elements->collect(x | x.shortName)->excludes('a')",
        &c
    ));
}

#[test]
fn ocl_includes_all_excludes_all() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(ev(
        "self.elements->collect(x | x.shortName)->includesAll(self.elements->select(x | x.shortName = 'a')->collect(y | y.shortName))",
        &c
    ));
    assert!(!ev(
        "self.elements->select(x | x.shortName = 'a' or x.shortName = 'b')->collect(y | y.shortName)->includesAll(self.elements->collect(x | x.shortName))",
        &c
    ));
    assert!(ev(
        "self.elements->select(x | x.shortName = 'a')->collect(y | y.shortName)->excludesAll(self.elements->select(x | x.shortName = 'b')->collect(y | y.shortName))",
        &c
    ));
    assert!(!ev(
        "self.elements->select(x | x.shortName = 'a')->collect(y | y.shortName)->excludesAll(self.elements->select(x | x.shortName = 'a')->collect(y | y.shortName))",
        &c
    ));
}

#[test]
fn ocl_union_intersection_difference() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(ev(
        "self.elements->select(x | x.shortName = 'a' or x.shortName = 'b')->collect(y | y.shortName)->union(self.elements->select(x | x.shortName = 'b' or x.shortName = 'c')->collect(y | y.shortName))->size() = 4",
        &c
    ));
    assert!(ev(
        "self.elements->select(x | x.shortName = 'a' or x.shortName = 'b')->collect(y | y.shortName)->intersection(self.elements->select(x | x.shortName = 'b' or x.shortName = 'c')->collect(y | y.shortName))->size() = 1",
        &c
    ));
    assert!(ev(
        "self.elements->select(x | x.shortName = 'a' or x.shortName = 'b')->collect(y | y.shortName)->difference(self.elements->select(x | x.shortName = 'b' or x.shortName = 'c')->collect(y | y.shortName))->size() = 1",
        &c
    ));
    assert!(ev(
        "self.elements->select(x | x.shortName = 'b' or x.shortName = 'c')->collect(y | y.shortName)->difference(self.elements->select(x | x.shortName = 'a' or x.shortName = 'b')->collect(y | y.shortName))->includes('c')",
        &c
    ));
}

#[test]
fn ocl_flatten_nested_elists() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    let e0 = make_plain_element(&e_cls, "e0");
    let e1 = make_plain_element(&e_cls, "e1");
    set_children(
        &e0,
        vec![make_plain_element(&e_cls, "a"), make_plain_element(&e_cls, "b")],
    );
    set_children(&e1, vec![make_plain_element(&e_cls, "d")]);
    set_elements(&c, vec![e0, e1]);
    assert!(ev("self.elements->collect(x | x.children)->size() = 2", &c));
    assert!(ev(
        "self.elements->collect(x | x.children)->flatten()->size() = 3",
        &c
    ));
    assert!(ev(
        "self.elements->collect(x | x.shortName)->flatten()->size() = 2",
        &c
    ));
}

#[test]
fn ocl_sum_numeric_and_string() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_element(&e_cls, "a", 1),
            make_element(&e_cls, "b", 2),
            make_element(&e_cls, "c", 3),
        ],
    );
    assert!(ev("self.elements->collect(x | x.count)->sum() = 6", &c));
    assert!(ev(
        "self.elements->collect(x | x.shortName)->sum() = 'abc'",
        &c
    ));
    let empty = new_container(&c_cls);
    assert!(ev("self.elements->collect(x | x.count)->sum() = 0", &empty));
}

#[test]
fn ocl_as_set_deduplicates() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
        ],
    );
    assert!(ev("self.elements->collect(x | x.shortName)->size() = 4", &c));
    assert!(ev(
        "self.elements->collect(x | x.shortName)->asSet()->size() = 2",
        &c
    ));
    assert!(ev(
        "self.elements->collect(x | x.shortName)->asSet()->includes('a')",
        &c
    ));
}

#[test]
fn ocl_as_sequence_as_bag_preserve_elements() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
        ],
    );
    assert!(ev(
        "self.elements->collect(x | x.shortName)->asSequence()->size() = 2",
        &c
    ));
    assert!(ev(
        "self.elements->collect(x | x.shortName)->asBag()->size() = 2",
        &c
    ));
    assert!(ev(
        "self.elements->collect(x | x.shortName)->asOrderedSet()->size() = 2",
        &c
    ));
}

// ===== collection equality =====

#[test]
fn ocl_collection_equality_same_elements_is_equal() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(ev(
        "self.elements->collect(x | x.shortName) = self.elements->collect(x | x.shortName)",
        &c
    ));
}

#[test]
fn ocl_collection_equality_order_independent() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "c"),
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
        ],
    );
    assert!(ev(
        "self.elements->collect(x | x.shortName) = self.elements->sortedBy(x | x.shortName)->collect(y | y.shortName)",
        &c
    ));
}

#[test]
fn ocl_collection_inequality_different_elements() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "c"),
        ],
    );
    assert!(ev(
        "self.elements->collect(x | x.shortName) <> self.elements->select(x | x.shortName = 'a')->collect(y | y.shortName)",
        &c
    ));
    assert!(ev(
        "self.elements->collect(x | x.shortName) <> self.elements->collect(x | x.count)",
        &c
    ));
    assert!(!ev(
        "self.elements->collect(x | x.shortName) <> self.elements->collect(x | x.shortName)",
        &c
    ));
}

#[test]
fn ocl_collection_equality_duplicate_sensitive() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![
            make_plain_element(&e_cls, "a"),
            make_plain_element(&e_cls, "b"),
            make_plain_element(&e_cls, "a"),
        ],
    );
    assert!(ev(
        "self.elements->collect(x | x.shortName) <> self.elements->collect(x | x.shortName)->asSet()",
        &c
    ));
    assert!(ev(
        "self.elements->collect(x | x.shortName)->asSet() = self.elements->collect(x | x.shortName)->asSet()",
        &c
    ));
}

// ===== OCL Tuple =====

#[test]
fn ocl_tuple_literal_and_field_access() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("Tuple { a = 1, b = 'x' }.a = 1", &c));
    assert!(ev("Tuple { a = 1, b = 'x' }.b = 'x'", &c));
}

#[test]
fn ocl_tuple_literal_with_type_annotation() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("Tuple { a : Integer = 42, b : String = 'hi' }.a = 42", &c));
    assert!(ev("Tuple { a : Integer = 42, b : String = 'hi' }.b = 'hi'", &c));
}

#[test]
fn ocl_tuple_access_missing_part_is_null() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("Tuple { a = 1 }.missing = null", &c));
    assert!(ev("Tuple { a = 1 }.a <> null", &c));
}

#[test]
fn ocl_tuple_equality_same_parts_is_equal() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("Tuple { a = 1, b = 'x' } = Tuple { a = 1, b = 'x' }", &c));
}

#[test]
fn ocl_tuple_equality_order_independent() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("Tuple { a = 1, b = 'x' } = Tuple { b = 'x', a = 1 }", &c));
}

#[test]
fn ocl_tuple_inequality_different_values() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("Tuple { a = 1, b = 'x' } <> Tuple { a = 2, b = 'x' }", &c));
    assert!(ev("Tuple { a = 1, b = 'x' } <> Tuple { a = 1, b = 'y' }", &c));
}

#[test]
fn ocl_tuple_inequality_different_parts() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("Tuple { a = 1, b = 'x' } <> Tuple { a = 1, c = 'x' }", &c));
    assert!(ev("Tuple { a = 1, b = 'x' } <> Tuple { a = 1 }", &c));
}

#[test]
fn ocl_tuple_with_let_binding() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("let t = Tuple { a = 5, b = 10 } in t.a + t.b = 15", &c));
}

#[test]
fn ocl_tuple_in_collect_grouping() {
    let (c_cls, e_cls) = meta();
    let c = new_container(&c_cls);
    set_elements(
        &c,
        vec![make_element(&e_cls, "a", 1), make_element(&e_cls, "b", 2)],
    );
    assert!(ev(
        "self.elements->collect(x | Tuple { name = x.shortName, cnt = x.count })->size() = 2",
        &c
    ));
    assert!(ev(
        "self.elements->collect(x | Tuple { name = x.shortName, cnt = x.count })->first().name = 'a'",
        &c
    ));
}

#[test]
fn ocl_tuple_nested_tuple() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("Tuple { outer = Tuple { inner = 7 } }.outer.inner = 7", &c));
}

#[test]
fn ocl_tuple_empty_literal() {
    let (c_cls, _) = meta();
    let c = new_container(&c_cls);
    assert!(ev("Tuple {} = Tuple {}", &c));
}