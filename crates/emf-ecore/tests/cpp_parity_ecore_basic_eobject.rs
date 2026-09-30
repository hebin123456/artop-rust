//! C++ parity suite: emf-ecore BasicEObject dynamic value storage,
//! container back-link, inverse-list maintenance and change notification.
//!
//! Ports `BasicEObjectTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-ecore/tests/` against `DynamicEObject`.
//!
//! Rust maps the C++ *feature-pointer* `eDynamicGet/Set/IsSet/Unset(feature*)`
//! onto the *by-name* `e_get/e_set/e_is_set/e_unset`, and the C++
//! `EObject*`-based `eInverseAdd/Remove`/`setEContainer` onto the `ObjectRef`
//! (`Rc<RefCell<dyn EObject>>`) forms on the `EObject` trait.
//!
//! `eClass` default null is expressed via a bare `EObject` whose `e_class()`
//! is the empty name (see `dynamic_eobject::b_eclass_default_null`).
use emf_common::eobject::{EObject, InverseList};
use emf_common::notification::{Adapter, EventType, Notification};
use emf_common::value::ObjectRef;
use emf_ecore::{DynamicEObject, EClass, EClassKind, EStructuralFeature, Val};
use std::cell::RefCell;
use std::rc::Rc;

fn simple_class() -> EClass {
    let mut cls = EClass::new("Simple", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_feature_id(0);
    name.set_type_name("EString");
    cls.add_feature(name);
    cls
}

// ----- eClass -----

#[test]
fn e_class_returns_set_class() {
    let cls = simple_class();
    let obj = DynamicEObject::new(cls);
    assert_eq!(obj.class().name(), "Simple");
}

// ----- eDynamicGet -----

#[test]
fn e_dynamic_get_unset_returns_null() {
    let obj = DynamicEObject::new(simple_class());
    let v = obj.e_get_by_name("name").expect("known feature");
    assert_eq!(v, Val::Null); // unset -> nothing stored -> null
}

// ----- eDynamicSet / eDynamicGet -----

#[test]
fn e_dynamic_set_then_get() {
    let mut obj = DynamicEObject::new(simple_class());
    assert!(obj.e_set_by_name("name", Val::String("Alice".into())));
    let v = obj.e_get_by_name("name").expect("set value");
    assert!(matches!(&v, Val::String(s) if s == "Alice"));
}

#[test]
fn e_dynamic_set_overwrite() {
    let mut obj = DynamicEObject::new(simple_class());
    obj.e_set_by_name("name", Val::String("first".into()));
    obj.e_set_by_name("name", Val::String("second".into()));
    let v = obj.e_get_by_name("name").expect("set value");
    assert!(matches!(&v, Val::String(s) if s == "second"));
}

#[test]
fn e_dynamic_set_distinct_features() {
    let mut cls = EClass::new("Two", EClassKind::Class);
    let mut a = EStructuralFeature::attribute("a");
    a.set_feature_id(0);
    a.set_type_name("EString");
    let mut b = EStructuralFeature::attribute("b");
    b.set_feature_id(1);
    b.set_type_name("EInt");
    cls.add_feature(a);
    cls.add_feature(b);

    let mut obj = DynamicEObject::new(cls);
    obj.e_set_by_name("a", Val::String("hello".into()));
    obj.e_set_by_name("b", Val::Int(42));

    assert!(matches!(
        obj.e_get_by_name("a").expect("a"),
        Val::String(s) if s == "hello"
    ));
    assert_eq!(obj.e_get_by_name("b").expect("b"), Val::Int(42));
}

// ----- eDynamicIsSet -----

#[test]
fn e_dynamic_is_set_default_false() {
    let obj = DynamicEObject::new(simple_class());
    assert_eq!(obj.e_is_set_by_name("name"), Some(false));
}

#[test]
fn e_dynamic_is_set_after_set_true() {
    let mut obj = DynamicEObject::new(simple_class());
    obj.e_set_by_name("name", Val::String("x".into()));
    assert_eq!(obj.e_is_set_by_name("name"), Some(true));
}

// ----- eDynamicUnset -----

#[test]
fn e_dynamic_unset_clears_value() {
    let mut obj = DynamicEObject::new(simple_class());
    obj.e_set_by_name("name", Val::String("x".into()));
    assert_eq!(obj.e_is_set_by_name("name"), Some(true));
    obj.e_unset_by_name("name");
    assert_eq!(obj.e_is_set_by_name("name"), Some(false));
    assert_eq!(obj.e_get_by_name("name").expect("known"), Val::Null);
}

#[test]
fn e_dynamic_unset_unknown_feature_no_op() {
    // Mirrors the C++ "unset a feature that was never set": no panic.
    let mut obj = DynamicEObject::new(simple_class());
    let unknown = obj.e_unset_by_name("does-not-exist");
    assert!(!unknown);
    assert_eq!(obj.e_is_set_by_name("name"), Some(false));
}

#[test]
fn b_e_dynamic_null_feature_cases() {
    // Ports BasicEObject_EDynamicGet_NullFeature_ReturnsEmpty /
    // EDynamicSet_NullFeature_NoOp / EDynamicIsSet_NullFeature_False /
    // EDynamicUnset_NullFeature_NoOp: an unknown ("null") feature resolves to
    // None/false and the write/unset calls are harmless no-ops.
    let mut obj = DynamicEObject::new(simple_class());
    assert_eq!(obj.e_get_by_name("no_such_feature"), None);
    assert!(!obj.e_set_by_name("no_such_feature", Val::String("x".into())));
    assert_eq!(obj.e_is_set_by_name("no_such_feature"), None);
    assert!(!obj.e_unset_by_name("no_such_feature"));
}

// ===== test doubles =====

/// Adapter that records every delivered notification (C++ `RecordingAdapter2`).
struct RecordingAdapter {
    events: Rc<RefCell<Vec<Notification>>>,
}

impl RecordingAdapter {
    fn new(events: Rc<RefCell<Vec<Notification>>>) -> Box<dyn Adapter> {
        Box::new(Self { events })
    }
}

impl Adapter for RecordingAdapter {
    fn notify_changed(&mut self, notification: &Notification) {
        self.events.borrow_mut().push(notification.clone());
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Reverse-list stub recording `basic_add`/`basic_remove` (C++
/// `InverseListStub`). The backing store is shared so the test can inspect it.
#[derive(Default)]
struct InverseListStub {
    added: Rc<RefCell<Vec<ObjectRef>>>,
}

impl InverseList for InverseListStub {
    fn basic_add(&mut self, other_end: &ObjectRef) {
        self.added.borrow_mut().push(Rc::clone(other_end));
    }
    fn basic_remove(&mut self, other_end: &ObjectRef) {
        let mut added = self.added.borrow_mut();
        if let Some(pos) = added.iter().position(|o| Rc::ptr_eq(o, other_end)) {
            added.remove(pos);
        }
    }
}

/// A fresh registrable inverse list plus its observable backing store.
fn inverse_stub() -> (Rc<RefCell<dyn InverseList>>, Rc<RefCell<Vec<ObjectRef>>>) {
    let added: Rc<RefCell<Vec<ObjectRef>>> = Rc::new(RefCell::new(Vec::new()));
    let stub = InverseListStub {
        added: Rc::clone(&added),
    };
    (Rc::new(RefCell::new(stub)), added)
}

/// A fresh `DynamicEObject` as a shared `ObjectRef` (stands in for the C++
/// `TestBasicEObject` "other end" pointers).
fn object_ref() -> ObjectRef {
    Rc::new(RefCell::new(DynamicEObject::new(simple_class())))
}

// ===== eContainer =====

#[test]
fn b_e_container_default_none() {
    // BasicEObject_EContainer_DefaultNull
    let obj = DynamicEObject::new(simple_class());
    assert!(obj.e_container().is_none());
}

#[test]
fn b_e_container_after_set_e_container() {
    // BasicEObject_EContainer_AfterSetEContainer
    let parent = object_ref();
    let mut child = DynamicEObject::new(simple_class());
    child.set_e_container(Some(Rc::clone(&parent)));
    let got = child.e_container().expect("container is set");
    assert!(Rc::ptr_eq(&got, &parent));
    // setEContainer records no containment feature -> eContainingFeature is none.
    assert!(child.e_containing_feature().is_none());
}

#[test]
fn b_set_e_container_fires_reverse_add() {
    let events: Rc<RefCell<Vec<Notification>>> = Rc::new(RefCell::new(Vec::new()));
    let mut child = DynamicEObject::new(simple_class());
    child.add_adapter(RecordingAdapter::new(Rc::clone(&events)));
    let parent = object_ref();
    child.set_e_container(Some(Rc::clone(&parent)));
    let rec = events.borrow();
    assert_eq!(rec.len(), 1);
    assert_eq!(rec[0].event(), EventType::Add);
    assert!(rec[0].feature().is_none()); // reverse path carries no feature
    assert!(matches!(&rec[0].new_value, Val::Object(o) if Rc::ptr_eq(o, &parent)));
}

#[test]
fn b_set_e_container_switch_fires_remove_then_add() {
    let events: Rc<RefCell<Vec<Notification>>> = Rc::new(RefCell::new(Vec::new()));
    let mut child = DynamicEObject::new(simple_class());
    child.add_adapter(RecordingAdapter::new(Rc::clone(&events)));
    let p1 = object_ref();
    let p2 = object_ref();
    child.set_e_container(Some(Rc::clone(&p1))); // ADD(p1)
    child.set_e_container(Some(Rc::clone(&p2))); // REMOVE(p1) + ADD(p2)
    let rec = events.borrow();
    assert_eq!(rec.len(), 3);
    assert_eq!(rec[1].event(), EventType::Remove);
    assert!(matches!(&rec[1].old_value, Val::Object(o) if Rc::ptr_eq(o, &p1)));
    assert_eq!(rec[2].event(), EventType::Add);
    assert!(matches!(&rec[2].new_value, Val::Object(o) if Rc::ptr_eq(o, &p2)));
}

#[test]
fn b_set_e_container_same_container_no_notification() {
    let events: Rc<RefCell<Vec<Notification>>> = Rc::new(RefCell::new(Vec::new()));
    let mut child = DynamicEObject::new(simple_class());
    child.add_adapter(RecordingAdapter::new(Rc::clone(&events)));
    let parent = object_ref();
    child.set_e_container(Some(Rc::clone(&parent)));
    child.set_e_container(Some(Rc::clone(&parent))); // unchanged
    assert_eq!(events.borrow().len(), 1);
}

// ===== eRegisterInverseList / eUnregisterInverseList / eInverseAdd / eInverseRemove =====

#[test]
fn b_register_inverse_list_enables_inverse_add() {
    // BasicEObject_ERegisterInverseList_EnablesInverseAdd
    let mut owner = DynamicEObject::new(simple_class());
    let (list, added) = inverse_stub();
    owner.e_register_inverse_list(7, Some(list));

    let other = object_ref();
    let chain = owner.e_inverse_add(&other, 7, Vec::new());
    assert_eq!(added.borrow().len(), 1);
    assert!(Rc::ptr_eq(&added.borrow()[0], &other));
    // No adapters on `owner` -> no notification appended to the chain.
    assert!(chain.is_empty());
}

#[test]
fn b_e_inverse_add_unregistered_feature_no_op() {
    // BasicEObject_EInverseAdd_UnregisteredFeature_NoOp
    let owner = DynamicEObject::new(simple_class());
    let other = object_ref();
    let chain = owner.e_inverse_add(&other, 999, Vec::new());
    assert!(chain.is_empty());
}

#[test]
fn b_e_inverse_remove_removes_from_list() {
    // BasicEObject_EInverseRemove_RemovesFromList
    let mut owner = DynamicEObject::new(simple_class());
    let (list, added) = inverse_stub();
    owner.e_register_inverse_list(3, Some(list));

    let a = object_ref();
    let b = object_ref();
    owner.e_inverse_add(&a, 3, Vec::new());
    owner.e_inverse_add(&b, 3, Vec::new());
    assert_eq!(added.borrow().len(), 2);

    owner.e_inverse_remove(&a, 3, Vec::new());
    assert_eq!(added.borrow().len(), 1);
    assert!(!added.borrow().iter().any(|o| Rc::ptr_eq(o, &a)));
    assert!(added.borrow().iter().any(|o| Rc::ptr_eq(o, &b)));
}

#[test]
fn b_e_inverse_remove_unregistered_feature_no_op() {
    // BasicEObject_EInverseRemove_UnregisteredFeature_NoOp
    let owner = DynamicEObject::new(simple_class());
    let other = object_ref();
    let chain = owner.e_inverse_remove(&other, 500, Vec::new());
    assert!(chain.is_empty());
}

#[test]
fn b_e_unregister_inverse_list_disables_inverse_add() {
    // BasicEObject_EUnregisterInverseList_DisablesInverseAdd
    let mut owner = DynamicEObject::new(simple_class());
    let (list, added) = inverse_stub();
    owner.e_register_inverse_list(1, Some(Rc::clone(&list)));
    owner.e_unregister_inverse_list(1, &list);

    let other = object_ref();
    owner.e_inverse_add(&other, 1, Vec::new());
    assert_eq!(added.borrow().len(), 0);
}

#[test]
fn b_e_unregister_inverse_list_wrong_list_no_op() {
    // BasicEObject_EUnregisterInverseList_WrongList_NoOp
    let mut owner = DynamicEObject::new(simple_class());
    let (list1, added1) = inverse_stub();
    let (list2, _added2) = inverse_stub();
    owner.e_register_inverse_list(2, Some(list1));
    // A mismatched list must not remove the registered one.
    owner.e_unregister_inverse_list(2, &list2);

    let other = object_ref();
    owner.e_inverse_add(&other, 2, Vec::new());
    assert_eq!(added1.borrow().len(), 1);
}

#[test]
fn b_register_inverse_list_null_no_op() {
    // BasicEObject_ERegisterInverseList_NullList_NoOp
    let mut owner = DynamicEObject::new(simple_class());
    owner.e_register_inverse_list(0, None);
    let other = object_ref();
    owner.e_inverse_add(&other, 0, Vec::new());
}

#[test]
fn b_e_inverse_add_passes_through_notifications() {
    // BasicEObject_EInverseAdd_PassesThroughNotifications
    let mut owner = DynamicEObject::new(simple_class());
    let (list, _added) = inverse_stub();
    owner.e_register_inverse_list(4, Some(list));

    let chain = vec![Notification::new(
        EventType::Add,
        None,
        Val::Null,
        Val::Null,
        4,
        false,
    )];
    let other = object_ref();
    let result = owner.e_inverse_add(&other, 4, chain);
    assert_eq!(result.len(), 1); // original chain passed through
}

#[test]
fn b_e_inverse_remove_passes_through_notifications() {
    // BasicEObject_EInverseRemove_PassesThroughNotifications
    let mut owner = DynamicEObject::new(simple_class());
    let (list, _added) = inverse_stub();
    owner.e_register_inverse_list(4, Some(list));

    let other = object_ref();
    let result = owner.e_inverse_remove(&other, 4, Vec::new());
    assert!(result.is_empty()); // empty chain passed through
}

#[test]
fn b_e_inverse_add_with_adapter_appends_notification() {
    // With adapters present, eInverseAdd appends a reverse ADD labeled by the
    // resolved feature name (here the class's feature id 0 == "name").
    let mut owner = DynamicEObject::new(simple_class());
    let (list, added) = inverse_stub();
    owner.e_register_inverse_list(0, Some(list));
    let events: Rc<RefCell<Vec<Notification>>> = Rc::new(RefCell::new(Vec::new()));
    owner.add_adapter(RecordingAdapter::new(events));

    let other = object_ref();
    let chain = owner.e_inverse_add(&other, 0, Vec::new());
    assert_eq!(added.borrow().len(), 1);
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0].event(), EventType::Add);
    assert_eq!(chain[0].feature(), Some("name"));
}

// ===== eNotificationRequired =====

#[test]
fn b_e_notification_required_no_adapters_false() {
    // BasicEObject_ENotificationRequired_NoAdapters_False
    let obj = DynamicEObject::new(simple_class());
    assert!(!obj.e_notification_required());
}

#[test]
fn b_e_notification_required_with_adapter_true() {
    let obj = DynamicEObject::new(simple_class());
    let events: Rc<RefCell<Vec<Notification>>> = Rc::new(RefCell::new(Vec::new()));
    obj.add_adapter(RecordingAdapter::new(events));
    assert!(obj.e_notification_required());
}

// ===== eSet / eUnset fire SET / UNSET and read back =====

#[test]
fn b_e_set_fires_set_and_reads_back() {
    // BasicEObject_ESet_Feature_FiresSetAndReadsBack
    let mut obj = DynamicEObject::new(simple_class());
    obj.e_set_by_name("name", Val::String("first".into())); // no adapter yet

    let events: Rc<RefCell<Vec<Notification>>> = Rc::new(RefCell::new(Vec::new()));
    obj.add_adapter(RecordingAdapter::new(Rc::clone(&events)));

    obj.e_set_by_name("name", Val::String("second".into()));
    {
        let rec = events.borrow();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].event(), EventType::Set);
        assert_eq!(rec[0].feature(), Some("name"));
        assert_eq!(rec[0].old_value, Val::String("first".into()));
        assert_eq!(rec[0].new_value, Val::String("second".into()));
    }
    // Value reads back through eGet.
    assert_eq!(
        obj.e_get_by_name("name"),
        Some(Val::String("second".into()))
    );
}

#[test]
fn b_e_unset_fires_unset() {
    // BasicEObject_EUnset_Feature_FiresUnsetNotification
    let mut obj = DynamicEObject::new(simple_class());
    obj.e_set_by_name("name", Val::String("x".into()));

    let events: Rc<RefCell<Vec<Notification>>> = Rc::new(RefCell::new(Vec::new()));
    obj.add_adapter(RecordingAdapter::new(Rc::clone(&events)));

    obj.e_unset_by_name("name");
    let rec = events.borrow();
    assert_eq!(rec.len(), 1);
    assert_eq!(rec[0].event(), EventType::Unset);
    drop(rec);
    assert_eq!(obj.e_is_set_by_name("name"), Some(false));
    assert_eq!(obj.e_get_by_name("name"), Some(Val::Null));
}

// ===== framework smoke test =====

#[test]
fn placeholder() {
    // C++ `EMF_TEST(Placeholder)` in `test_main.cpp` is the empty framework
    // smoke test (`EXPECT_TRUE(true)`); ported verbatim as a no-op.
    assert!(true);
}
