//! Rust port parity tests for `EObjectEListTests.cpp` (C++ `emf-ecore-util`,
//! aligned to Java `org.eclipse.emf.ecore.util.EObjectEList`).
//!
//! Ports the whole file: construction (feature id / owner / data class),
//! `EcoreEList` hooks, add/addUnique/addAllUnique, unique & null constraints,
//! get/basicGet, contains/indexOf, remove(index)/remove(value), setUnique,
//! clear, move, toArray and isSet/unset.

use emf_common::value::ObjectRef;
use emf_ecore::{DynamicEObject, EClass, EClassKind};
use emf_ecore_util::e_object_elist::EObjectEList;
use std::cell::RefCell;
use std::rc::Rc;

// ---------- helpers ----------

fn element_class() -> EClass {
    EClass::new("Element", EClassKind::Class)
}

fn make_element(cls: &EClass) -> ObjectRef {
    Rc::new(RefCell::new(DynamicEObject::new(cls.clone())))
}

// ===== 构造与基本属性 =====
#[test]
fn construct_stores_feature_id() {
    let cls = element_class();
    let list = EObjectEList::new(cls, 7);
    assert_eq!(list.get_feature_id(), 7);
    assert_eq!(list.size(), 0);
    assert!(list.is_empty());
}

#[test]
fn construct_feature_id_kept() {
    let cls = element_class();
    let list = EObjectEList::new(cls, 5);
    assert_eq!(list.get_feature_id(), 5);
}

#[test]
fn owner_data_class_stored() {
    let cls = element_class();
    let list = EObjectEList::new(cls.clone(), 3);
    assert_eq!(list.data_class().name(), "Element");
    assert!(list.owner().is_none());
}

// ===== EcoreEList 钩子 =====
#[test]
fn use_equals_false() {
    let list = EObjectEList::new(element_class(), 0);
    assert!(!list.use_equals());
}

#[test]
fn is_unique_true() {
    let list = EObjectEList::new(element_class(), 0);
    assert!(list.is_unique());
}

#[test]
fn has_inverse_false() {
    let list = EObjectEList::new(element_class(), 0);
    assert!(!list.has_inverse());
}

#[test]
fn is_e_object_true() {
    let list = EObjectEList::new(element_class(), 0);
    assert!(list.is_e_object());
}

#[test]
fn can_contain_null_false() {
    let list = EObjectEList::new(element_class(), 0);
    assert!(!list.can_contain_null());
}

// ===== add / size =====
#[test]
fn add_increases_size() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    assert!(list.add(a));
    assert_eq!(list.size(), 1);
    assert!(!list.is_empty());
}

#[test]
fn add_multiple_preserves_order() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    let b = make_element(&cls);
    let c = make_element(&cls);
    list.add(a.clone());
    list.add(b.clone());
    list.add(c.clone());
    assert_eq!(list.size(), 3);
    assert!(object_eq(&list.get(0), &a));
    assert!(object_eq(&list.get(1), &b));
    assert!(object_eq(&list.get(2), &c));
}

#[test]
fn add_duplicate_rejected() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    assert!(list.add(a.clone()));
    assert!(!list.add(a)); // duplicate pointer rejected
    assert_eq!(list.size(), 1);
}

#[test]
fn add_unique_duplicate_not_checked() {
    // addUnique bypasses uniqueness (Java semantics: caller guarantees unique).
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    list.add_unique(a.clone());
    list.add_unique(a.clone());
    assert_eq!(list.size(), 2);
}

// ===== add 拒绝 null（canContainNull=false）=====
// ObjectRef is not nullable by construction; a list element is always a real
// object. Constructing a list with no elements is still valid, so we assert
// the null-invariant the same way C++ throws: there is no way to obtain a
// `null` ObjectRef, hence the constraint is structurally guaranteed.
#[test]
fn add_rejects_null_by_construction() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls, 0);
    // No `None` exists in ObjectRef's type; appending a real object works,
    // and the empty list remains valid — mirroring that null cannot be stored.
    assert!(list.is_empty());
    let a = make_element(&element_class());
    assert!(list.add(a));
}

// ===== get / basicGet =====
#[test]
fn get_in_bounds() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    let b = make_element(&cls);
    list.add(a.clone());
    list.add(b.clone());
    assert!(object_eq(&list.get(0), &a));
    assert!(object_eq(&list.get(1), &b));
}

#[test]
#[should_panic]
fn get_out_of_bounds_panics() {
    let cls = element_class();
    let list = EObjectEList::new(cls, 0);
    let _ = list.get(0);
}

#[test]
fn basic_get_returns_stored() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    list.add(a.clone());
    assert!(object_eq(&list.basic_get(0), &a));
}

// ===== contains / indexOf =====
#[test]
fn contains_present_true() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    list.add(a.clone());
    assert!(list.contains(&a));
}

#[test]
fn contains_absent_false() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    assert!(!list.contains(&a));
}

#[test]
fn index_of_found() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    let b = make_element(&cls);
    list.add(a.clone());
    list.add(b.clone());
    assert_eq!(list.index_of(&a), 0);
    assert_eq!(list.index_of(&b), 1);
}

#[test]
fn index_of_not_found_negative_one() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls, 0);
    let a = make_element(&element_class());
    assert_eq!(list.index_of(&a), -1);
}

// ===== remove(int) =====
#[test]
fn remove_by_index_returns_old() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    let b = make_element(&cls);
    list.add(a.clone());
    list.add(b.clone());
    let removed = list.remove_index(0);
    assert!(object_eq(&removed, &a));
    assert_eq!(list.size(), 1);
    assert!(object_eq(&list.get(0), &b));
}

#[test]
#[should_panic]
fn remove_by_index_out_of_bounds_panics() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls, 0);
    let _ = list.remove_index(0);
}

// ===== remove(value) =====
#[test]
fn remove_value_returns_true() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    let b = make_element(&cls);
    list.add(a.clone());
    list.add(b.clone());
    assert!(list.remove_value(&a));
    assert_eq!(list.size(), 1);
    assert!(!list.contains(&a));
    assert!(list.contains(&b));
}

#[test]
fn remove_value_not_present_returns_false() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    list.add(a.clone());
    let b = make_element(&cls); // not added
    assert!(!list.remove_value(&b));
    assert_eq!(list.size(), 1);
}

// ===== set =====
#[test]
fn set_unique_replaces_old() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    let b = make_element(&cls);
    list.add(a.clone());
    let old = list.set_unique(0, b.clone());
    assert!(object_eq(&old, &a));
    assert!(object_eq(&list.get(0), &b));
}

#[test]
fn set_duplicate_at_other_index_panics() {
    // isUnique=true: putting an element already present at another slot rejects.
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    let b = make_element(&cls);
    list.add(a.clone());
    list.add(b);
    // `a` is already at index 0; a unique-`set` of `a` at index 1 is invalid.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = list.set_index_checked(1, a);
    }));
    assert!(result.is_err());
}

// ===== clear =====
#[test]
fn clear_empties_list() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    let b = make_element(&cls);
    list.add(a);
    list.add(b);
    list.clear();
    assert_eq!(list.size(), 0);
    assert!(list.is_empty());
}

// ===== move =====
#[test]
fn move_reorders() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    let b = make_element(&cls);
    let c = make_element(&cls);
    list.add(a.clone());
    list.add(b.clone());
    list.add(c.clone());
    // Move index 0 to index 2.
    let moved = list.move_to(2, 0);
    assert!(object_eq(&moved, &a));
    assert!(object_eq(&list.get(0), &b));
    assert!(object_eq(&list.get(1), &c));
    assert!(object_eq(&list.get(2), &a));
}

// ===== toArray =====
#[test]
fn to_array_returns_elements() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    let b = make_element(&cls);
    list.add(a.clone());
    list.add(b.clone());
    let arr = list.to_array();
    assert_eq!(arr.len(), 2);
    assert!(object_eq(&arr[0], &a));
    assert!(object_eq(&arr[1], &b));
}

// ===== isSet / unset =====
#[test]
fn is_set_true_when_non_empty() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    assert!(!list.is_set());
    let a = make_element(&cls);
    list.add(a);
    assert!(list.is_set());
    list.unset();
    assert!(!list.is_set());
    assert_eq!(list.size(), 0);
}

// ===== addAllUnique（批量添加）=====
#[test]
fn add_all_unique_appends_all() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls.clone(), 0);
    let a = make_element(&cls);
    let b = make_element(&cls);
    let input = vec![a.clone(), b.clone()];
    assert!(list.add_all_unique(input));
    assert_eq!(list.size(), 2);
    assert!(object_eq(&list.get(0), &a));
    assert!(object_eq(&list.get(1), &b));
}

#[test]
fn add_all_unique_empty_input_returns_false() {
    let cls = element_class();
    let mut list = EObjectEList::new(cls, 0);
    assert!(!list.add_all_unique(Vec::new()));
}

// ===== owner 保留 =====
#[test]
fn with_owner_keeps_backlink() {
    let cls = element_class();
    let owner = make_element(&cls);
    let list = EObjectEList::with_owner(cls, owner.clone(), 9);
    assert_eq!(list.get_feature_id(), 9);
    let up = list.owner().expect("owner should resolve while alive");
    assert!(Rc::ptr_eq(&owner, &up));
}

// ---------- helper：ObjectRef 按指针比较 ----------
fn object_eq(a: &ObjectRef, b: &ObjectRef) -> bool {
    Rc::ptr_eq(a, b)
}