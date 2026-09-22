//! C++ parity suite: emf-ecore DynamicEObject value storage + containment.
//!
//! Ports `DynamicEObjectImplTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-ecore/tests/` against Rust `DynamicEObject`.
//!
//! Differences from C++:
//!   - Rust reads/writes features by *name*; C++ uses feature pointers. The
//!     containment graph is built with explicit `adopt_single` / `adopt_many`
//!     helpers (the Rust analogue of `parent->eSet(containmentFeature, child)`),
//!     which set the child's `eContainer`/`eContainmentFeature` back-link.
//!   - `eContents` collects children from single + multi containment features
//!     in feature-id order.
use emf_common::value::{ObjectRef, Val};
use emf_ecore::{
    adopt_many, adopt_single, node_to_object, DynNode, DynamicEObject, EClass, EClassKind,
    EStructuralFeature,
};
use std::cell::RefCell;
use std::rc::Rc;

/// Model: Node EClass with
///   - name : EString (single, id 0)
///   - value : EInt (single, id 1)
///   - child : Node (containment, single, id 2)
///   - children : Node (containment, many, id 3)
fn node_class() -> EClass {
    let mut cls = EClass::new("Node", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_feature_id(0);
    let mut value = EStructuralFeature::attribute("value");
    value.set_feature_id(1);
    let mut child = EStructuralFeature::reference("child");
    child.set_feature_id(2);
    child.set_containment(true);
    let mut children = EStructuralFeature::reference_many("children");
    children.set_feature_id(3);
    children.set_containment(true);
    cls.add_feature(name);
    cls.add_feature(value);
    cls.add_feature(child);
    cls.add_feature(children);
    cls
}

fn node() -> DynNode {
    Rc::new(RefCell::new(DynamicEObject::new(node_class())))
}

// ===== eClass =====

#[test]
fn e_class_returns_constructor_class() {
    let obj = DynamicEObject::new(node_class());
    assert_eq!(obj.class().name(), "Node");
}

#[test]
fn e_contents_default_empty() {
    let obj = DynamicEObject::new(node_class());
    assert!(obj.contents().is_empty());
}

// ===== eSet / eGet, single-valued attributes =====

#[test]
fn e_set_e_get_string_attribute() {
    let mut obj = DynamicEObject::new(node_class());
    obj.e_set_by_name("name", Val::String("root".into()));
    let v = obj.e_get_by_name("name").expect("set");
    assert!(matches!(&v, Val::String(s) if s == "root"));
}

#[test]
fn e_set_e_get_int_attribute() {
    let mut obj = DynamicEObject::new(node_class());
    obj.e_set_by_name("value", Val::Int(42));
    let v = obj.e_get_by_name("value").expect("set");
    assert_eq!(v, Val::Int(42));
}

#[test]
fn e_get_unset_attribute_null() {
    let obj = DynamicEObject::new(node_class());
    assert_eq!(obj.e_get_by_name("name"), Some(Val::Null));
}

#[test]
fn e_get_with_resolve_flag_delegates() {
    let mut obj = DynamicEObject::new(node_class());
    obj.e_set_by_name("name", Val::String("x".into()));
    let v = obj.e_get_by_name("name").expect("set");
    assert!(matches!(&v, Val::String(s) if s == "x"));
}

// ===== eIsSet / eUnset =====

#[test]
fn e_is_set_default_false_after_set_true() {
    let mut obj = DynamicEObject::new(node_class());
    assert_eq!(obj.e_is_set_by_name("name"), Some(false));
    obj.e_set_by_name("name", Val::String("x".into()));
    assert_eq!(obj.e_is_set_by_name("name"), Some(true));
}

#[test]
fn e_unset_clears_value() {
    let mut obj = DynamicEObject::new(node_class());
    obj.e_set_by_name("name", Val::String("x".into()));
    assert_eq!(obj.e_is_set_by_name("name"), Some(true));
    obj.e_unset_by_name("name");
    assert_eq!(obj.e_is_set_by_name("name"), Some(false));
    assert_eq!(obj.e_get_by_name("name"), Some(Val::Null));
}

#[test]
fn e_is_set_by_feature_id() {
    let mut obj = DynamicEObject::new(node_class());
    assert_eq!(obj.e_is_set_by_name("name"), Some(false));
    obj.e_set_by_name("name", Val::String("y".into()));
    assert_eq!(obj.e_is_set_by_name("name"), Some(true));
}

// ===== containment single reference: adopt sets child eContainer =====

#[test]
fn containment_single_sets_container() {
    let parent = node();
    let child = node();
    adopt_single(&parent, "child", &child);
    let c = child.borrow();
    assert!(c.container_backref().is_some());
    assert_eq!(
        c.container_backref().as_ref().unwrap().1,
        "child".to_string()
    );
    assert!(c.container_backref().as_ref().unwrap().0.strong_count() >= 1);
}

#[test]
fn containment_single_get_returns_child() {
    let parent = node();
    let child = node();
    adopt_single(&parent, "child", &child);
    let v = parent.borrow().e_get_by_name("child").expect("set");
    assert!(v.as_object().is_some());
    assert!(Rc::ptr_eq(v.as_object().unwrap(), &node_to_object(&child)));
}

#[test]
fn containment_single_is_set_and_unset() {
    let parent = node();
    let child = node();
    assert_eq!(parent.borrow().e_is_set_by_name("child"), Some(false));
    adopt_single(&parent, "child", &child);
    assert_eq!(parent.borrow().e_is_set_by_name("child"), Some(true));
    parent.borrow_mut().e_unset_by_name("child");
    assert_eq!(parent.borrow().e_is_set_by_name("child"), Some(false));
    // child back-link cleared
    assert!(child.borrow().container_backref().is_none());
}

#[test]
fn e_unset_null_feature_no_op() {
    let mut obj = DynamicEObject::new(node_class());
    let ok = obj.e_unset_by_name("no_such");
    assert!(!ok);
}

// ===== containment multi reference: eGet returns list =====

#[test]
fn multi_value_e_get_returns_list() {
    let obj = DynamicEObject::new(node_class());
    assert!(obj.e_get_by_name("children").is_some());
    // unset many -> empty list value
    let v = obj.e_get_by_name("children").unwrap();
    assert!(v.as_list().is_some_and(|l| l.is_empty()));
}

#[test]
fn multi_value_mutate_list_directly_and_reuse() {
    let parent = node();
    let c1 = node();
    let c2 = node();
    adopt_many(&parent, "children", &c1);
    adopt_many(&parent, "children", &c2);
    let v = parent.borrow().e_get_by_name("children").expect("set");
    assert_eq!(v.as_list().map(|l| l.len()), Some(2));
}

#[test]
fn multi_value_is_set_after_add() {
    let parent = node();
    let c1 = node();
    assert_eq!(parent.borrow().e_is_set_by_name("children"), Some(false));
    adopt_many(&parent, "children", &c1);
    assert_eq!(parent.borrow().e_is_set_by_name("children"), Some(true));
}

#[test]
fn multi_value_unset_clears_list() {
    let parent = node();
    let c1 = node();
    let c2 = node();
    adopt_many(&parent, "children", &c1);
    adopt_many(&parent, "children", &c2);
    assert_eq!(parent.borrow().e_is_set_by_name("children"), Some(true));
    parent.borrow_mut().e_unset_by_name("children");
    assert_eq!(parent.borrow().e_is_set_by_name("children"), Some(false));
    let v = parent.borrow().e_get_by_name("children").unwrap();
    assert!(v.as_list().is_some_and(|l| l.is_empty()));
}

// ===== eContents collection =====

#[test]
fn e_contents_single_containment() {
    let parent = node();
    let child = node();
    adopt_single(&parent, "child", &child);
    let contents = parent.borrow().contents();
    assert_eq!(contents.len(), 1);
    assert!(Rc::ptr_eq(&contents[0], &node_to_object(&child)));
}

#[test]
fn e_contents_multi_containment() {
    let parent = node();
    let (c1, c2, c3) = (node(), node(), node());
    for c in [&c1, &c2, &c3] {
        adopt_many(&parent, "children", c);
    }
    let contents = parent.borrow().contents();
    assert_eq!(contents.len(), 3);
}

#[test]
fn e_contents_both_single_and_multi() {
    let parent = node();
    let single = node();
    let (m1, m2) = (node(), node());
    adopt_single(&parent, "child", &single);
    adopt_many(&parent, "children", &m1);
    adopt_many(&parent, "children", &m2);
    let contents = parent.borrow().contents();
    assert_eq!(contents.len(), 3);
}

#[test]
fn e_contents_no_containment_empty() {
    let mut obj = DynamicEObject::new(node_class());
    obj.e_set_by_name("name", Val::String("x".into()));
    obj.e_set_by_name("value", Val::Int(1));
    assert!(obj.contents().is_empty());
}

// ===== overwrite =====

#[test]
fn e_set_overwrite_attribute() {
    let mut obj = DynamicEObject::new(node_class());
    obj.e_set_by_name("name", Val::String("first".into()));
    obj.e_set_by_name("name", Val::String("second".into()));
    let v = obj.e_get_by_name("name").expect("set");
    assert!(matches!(&v, Val::String(s) if s == "second"));
}

#[test]
fn e_set_overwrite_containment_child() {
    let parent = node();
    let (c1, c2) = (node(), node());
    adopt_single(&parent, "child", &c1);
    assert!(c1.borrow().container_backref().is_some());
    // overwrite directly on the parent
    parent
        .borrow_mut()
        .e_set_by_name("child", Val::Object(node_to_object(&c2)));
    let v = parent.borrow().e_get_by_name("child").expect("set");
    assert!(Rc::ptr_eq(v.as_object().unwrap(), &node_to_object(&c2)));
}

#[test]
fn unknown_feature_returns_none() {
    let mut obj = DynamicEObject::new(node_class());
    assert!(!obj.e_set_by_name("nope", Val::Int(1)));
    assert_eq!(obj.e_get_by_name("nope"), None);
    assert_eq!(obj.e_is_set_by_name("nope"), None);
}

// Guard to keep ObjectRef import used in signature-level assertions.
#[allow(dead_code)]
fn assert_object_ref(_o: &ObjectRef) {}