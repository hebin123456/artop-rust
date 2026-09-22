//! C++ parity suite: emf-common core data structures.
//!
//! Ports `EListTests.cpp`, `UniqueEListTests.cpp` and `BasicEMapTests.cpp`
//! from `artop-cpp/cpp/emf-cpp/emf-common/tests/`. The C++ uses virtual
//! `EList<int>`; Rust models the same semantics with [`BasicEList<T>`], its
//! `add_unique` / `index_of` / `remove_value` covering C++ `UniqueEList`, and
//! [`BasicEMap`] for the map.
use emf_common::elist::{BasicEList, EList, ListChange};
use emf_common::emap::BasicEMap;
use std::cell::RefCell;
use std::rc::Rc;

// ---------------------------------------------------------------------------
// EListTests.cpp
// ---------------------------------------------------------------------------

#[test]
fn elist_add_get_basic() {
    let mut list = BasicEList::new();
    list.add(1);
    list.add(2);
    list.add(3);
    assert_eq!(list.len(), 3);
    assert_eq!(*list.get(0).unwrap(), 1);
    assert_eq!(*list.get(1).unwrap(), 2);
    assert_eq!(*list.get(2).unwrap(), 3);
}

#[test]
fn elist_operator_bracket() {
    let mut list = BasicEList::new();
    list.add(10);
    list.add(20);
    assert_eq!(*list.get(0).unwrap(), 10);
    assert_eq!(*list.get(1).unwrap(), 20);
    list.set(1, 99);
    assert_eq!(*list.get(1).unwrap(), 99);
}

#[test]
fn elist_remove_by_index() {
    let mut list = BasicEList::new();
    list.add(1);
    list.add(2);
    list.add(3);
    assert_eq!(list.remove(1), Some(2));
    assert_eq!(list.len(), 2);
    assert_eq!(*list.get(0).unwrap(), 1);
    assert_eq!(*list.get(1).unwrap(), 3);
    // C++ throws on out-of-range; Rust returns None.
    assert_eq!(list.remove(99), None);
}

#[test]
fn elist_remove_by_value() {
    let mut list = BasicEList::new();
    list.add(10);
    list.add(20);
    list.add(30);
    assert!(list.remove_value(&20));
    assert_eq!(list.len(), 2);
    assert!(!list.remove_value(&99));
}

#[test]
fn elist_set() {
    let mut list = BasicEList::new();
    list.add(1);
    list.add(2);
    assert_eq!(list.set(0, 42), Some(1));
    assert_eq!(*list.get(0).unwrap(), 42);
    assert_eq!(list.set(99, 0), None);
}

#[test]
fn elist_contains_and_index_of() {
    let mut list: BasicEList<String> = BasicEList::new();
    list.add("a".into());
    list.add("b".into());
    list.add("c".into());
    assert!(list.contains(&"b".to_string()));
    assert!(!list.contains(&"z".to_string()));
    assert_eq!(list.index_of(&"a".to_string()), Some(0));
    assert_eq!(list.index_of(&"c".to_string()), Some(2));
    assert_eq!(list.index_of(&"not-there".to_string()), None);
}

#[test]
fn elist_add_unique() {
    let mut list = BasicEList::new();
    assert!(list.add_unique(1));
    assert!(list.add_unique(2));
    assert!(!list.add_unique(1)); // duplicate
    assert!(!list.add_unique(2)); // duplicate
    assert_eq!(list.len(), 2);
    assert!(list.contains(&1));
    assert!(list.contains(&2));
}

#[test]
fn elist_notifier_callbacks() {
    let counts = Rc::new(RefCell::new((
        0_usize, 0_usize, 0_usize, 0_i64, 0_i64, 0_i64,
    )));
    let rec = counts.clone();
    let mut list = BasicEList::new();
    list.set_change_hook(Some(Box::new(move |ch| match ch {
        ListChange::Add { index } => {
            let mut c = rec.borrow_mut();
            c.0 += 1;
            c.3 = index as i64;
        }
        ListChange::Remove { index } => {
            let mut c = rec.borrow_mut();
            c.1 += 1;
            c.4 = index as i64;
        }
        ListChange::Set { index } => {
            let mut c = rec.borrow_mut();
            c.2 += 1;
            c.5 = index as i64;
        }
        ListChange::Move { .. } => {}
    })));

    list.add(1);
    list.add(2);
    list.add(3);
    {
        let c = counts.borrow();
        assert_eq!(c.0, 3);
        assert_eq!(c.3, 2);
    }
    list.set(1, 99);
    {
        let c = counts.borrow();
        assert_eq!(c.2, 1);
        assert_eq!(c.5, 1);
    }
    list.remove(0);
    {
        let c = counts.borrow();
        assert_eq!(c.1, 1);
        assert_eq!(c.4, 0);
    }
}

#[test]
fn elist_clear_empties() {
    let mut list = BasicEList::new();
    for i in 0..5 {
        list.add(i);
    }
    assert_eq!(list.len(), 5);
    list.clear();
    assert_eq!(list.len(), 0);
    assert!(list.is_empty());
}

#[test]
fn elist_iteration() {
    let mut list = BasicEList::new();
    list.add(10);
    list.add(20);
    list.add(30);
    assert_eq!(list.iter().sum::<i32>(), 60);
}

#[test]
fn elist_out_of_range_throws() {
    let mut list = BasicEList::new();
    list.add(1);
    assert_eq!(list.get(99), None);
}

// ---------------------------------------------------------------------------
// UniqueEListTests.cpp  (Rust models uniqueness with BasicEList + add_unique)
// ---------------------------------------------------------------------------

#[test]
fn unique_elist_add_unique() {
    let mut list = BasicEList::new();
    assert!(list.add_unique(1));
    assert!(list.add_unique(2));
    assert!(list.add_unique(3));
    assert!(!list.add_unique(2)); // duplicate rejected
    assert!(!list.add_unique(1));
    assert_eq!(list.len(), 3);
}

#[test]
fn unique_elist_add_all_drops_duplicates() {
    let mut list = BasicEList::new();
    for v in [1, 2, 2, 3, 1, 4, 4, 4] {
        list.add_unique(v);
    }
    assert_eq!(list.len(), 4);
    for v in [1, 2, 3, 4] {
        assert!(list.contains(&v));
    }
}

/// KNOWN DIVERGENCE (tracked, not a pass): C++ `UniqueEList.set` throws
/// `invalid_argument` when the replacement value is already present. Rust has
/// no dedicated `UniqueEList` type yet (only `BasicEList`), so this invariant
/// is not enforced. Ignored until a `UniqueEList` semantics lands.
#[test]
#[ignore = "Rust lacks dedicated UniqueEList; duplicate-set rejection pending"]
fn unique_elist_set_duplicate_is_rejected() {
    let mut list = BasicEList::new();
    list.add(1);
    list.add(2);
    let dup = *list.get(0).unwrap();
    list.set(1, dup);
}

#[test]
fn unique_elist_contains_and_index_of() {
    let mut list: BasicEList<String> = BasicEList::new();
    list.add("alpha".into());
    list.add("beta".into());
    list.add("gamma".into());
    assert!(list.contains(&"beta".to_string()));
    assert!(!list.contains(&"delta".to_string()));
    assert_eq!(list.index_of(&"alpha".to_string()), Some(0));
    assert_eq!(list.index_of(&"gamma".to_string()), Some(2));
    assert_eq!(list.index_of(&"delta".to_string()), None);
}

#[test]
fn unique_elist_constructor_from_collection() {
    // C++ UniqueEList(vector) de-duplicates on construction.
    let data = [1, 2, 2, 3, 1, 4];
    let mut list = BasicEList::default();
    for v in data {
        list.add_unique(v);
    }
    assert_eq!(list.len(), 4);
}

#[test]
fn unique_elist_remove_and_clear() {
    let mut list = BasicEList::new();
    list.add(1);
    list.add(2);
    list.add(3);
    assert!(list.remove_value(&1));
    assert_eq!(list.len(), 2);
    assert!(!list.contains(&1));
    list.clear();
    assert_eq!(list.len(), 0);
    assert!(list.is_empty());
}

#[test]
fn unique_elist_move() {
    let mut list = BasicEList::new();
    list.add(10);
    list.add(20);
    list.add(30);
    list.add(40);
    // move(0, 2): take element at index 2 (30) and put at index 0.
    let moved = list.move_element(0, 2).unwrap();
    assert_eq!(moved, 30);
    assert_eq!(*list.get(0).unwrap(), 30);
    assert_eq!(*list.get(1).unwrap(), 10);
    assert_eq!(*list.get(2).unwrap(), 20);
    assert_eq!(*list.get(3).unwrap(), 40);
}

// ---------------------------------------------------------------------------
// BasicEMapTests.cpp  (Rust BasicEMap)
// ---------------------------------------------------------------------------

#[test]
fn basic_emap_put_get() {
    let mut m = BasicEMap::new();
    m.put("a".to_string(), 1);
    m.put("b".to_string(), 2);
    assert_eq!(*m.get(&"a".to_string()).unwrap(), 1);
    assert_eq!(*m.get(&"b".to_string()).unwrap(), 2);
    assert_eq!(m.len(), 2);
}

#[test]
fn basic_emap_update_existing_key() {
    let mut m = BasicEMap::new();
    m.put("a".to_string(), 1);
    let old = m.put("a".to_string(), 99).unwrap();
    assert_eq!(old, 1);
    assert_eq!(*m.get(&"a".to_string()).unwrap(), 99);
    assert_eq!(m.len(), 1);
}

#[test]
fn basic_emap_contains_key_value() {
    let mut m = BasicEMap::new();
    m.put(1, "one".to_string());
    m.put(2, "two".to_string());
    assert!(m.contains_key(&1));
    assert!(!m.contains_key(&3));
    assert!(m.contains_value(&"two".to_string()));
    assert!(!m.contains_value(&"three".to_string()));
}

#[test]
fn basic_emap_remove_key() {
    let mut m = BasicEMap::new();
    m.put("a".to_string(), 1);
    m.put("b".to_string(), 2);
    m.put("c".to_string(), 3);
    assert!(m.remove_key(&"b".to_string()).is_some());
    assert_eq!(m.len(), 2);
    assert!(!m.contains_key(&"b".to_string()));
}

#[test]
fn basic_emap_index_of_key() {
    let mut m = BasicEMap::new();
    m.put("a".to_string(), 1);
    m.put("b".to_string(), 2);
    m.put("c".to_string(), 3);
    assert_eq!(m.index_of_key(&"a".to_string()), Some(0));
    assert_eq!(m.index_of_key(&"b".to_string()), Some(1));
    assert_eq!(m.index_of_key(&"c".to_string()), Some(2));
    assert_eq!(m.index_of_key(&"z".to_string()), None);
}

#[test]
fn basic_emap_clear() {
    let mut m = BasicEMap::new();
    m.put("a".to_string(), 1);
    m.put("b".to_string(), 2);
    m.clear();
    assert_eq!(m.len(), 0);
    assert!(!m.contains_key(&"a".to_string()));
}

#[test]
fn basic_emap_foreach_key_value() {
    let mut m = BasicEMap::new();
    m.put(1, "one".to_string());
    m.put(2, "two".to_string());
    m.put(3, "three".to_string());
    let key_sum: i32 = m.keys().sum();
    assert_eq!(key_sum, 6);
    let all_values: String = m.values().map(|s| s.clone()).collect();
    assert_eq!(all_values, "onetwothree");
}

#[test]
fn basic_emap_iteration() {
    let mut m = BasicEMap::new();
    m.put("a".to_string(), 1);
    m.put("b".to_string(), 2);
    let count = m.keys().count();
    assert_eq!(count, 2);
}
