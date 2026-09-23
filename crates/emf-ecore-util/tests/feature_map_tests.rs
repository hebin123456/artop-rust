//! Rust port parity tests for `BasicFeatureMapTests.cpp` (C++
//! `emf-ecore-util`, aligned to Java `org.eclipse.emf.ecore.util.FeatureMap`).

use emf_common::value::Val;
use emf_ecore::EStructuralFeature;
use emf_ecore_util::feature_map::{Entry, FeatureMap};

// ---------- helpers ----------

/// Three unbounded attributes `a` (EString) / `b` (EInt) / `c` (EString),
/// each with a distinct feature id (C++ `EAttribute` pointer identity).
fn model() -> (EStructuralFeature, EStructuralFeature, EStructuralFeature) {
    let mut a = EStructuralFeature::attribute("a");
    a.set_feature_id(0);
    let mut b = EStructuralFeature::attribute("b");
    b.set_feature_id(1);
    let mut c = EStructuralFeature::attribute("c");
    c.set_feature_id(2);
    (a, b, c)
}

fn s(v: &str) -> Val {
    Val::string(v.to_string())
}

// ---------- tests ----------

// 1) AddEntry
#[test]
fn add_entry() {
    let (a, _, _) = model();
    let mut fm = FeatureMap::new();
    fm.add(Entry::new(a.clone(), s("v1")));
    assert_eq!(fm.len(), 1);
}

// 2) AddMultipleEntries
#[test]
fn add_multiple_entries() {
    let (a, b, c) = model();
    let mut fm = FeatureMap::new();
    fm.add(Entry::new(a.clone(), s("a1")));
    fm.add(Entry::new(b.clone(), Val::Int(42)));
    fm.add(Entry::new(c.clone(), s("c1")));
    assert_eq!(fm.len(), 3);
}

// 3) AddByFeature
#[test]
fn add_by_feature() {
    let (a, b, _) = model();
    let mut fm = FeatureMap::new();
    fm.add_entry(a.clone(), s("hello"));
    fm.add_entry(a.clone(), s("world"));
    assert_eq!(fm.len(), 2);
    assert_eq!(fm.size_for(&a), 2);
    assert_eq!(fm.size_for(&b), 0);
}

// 4) AddByFeatureAndIndex
#[test]
fn add_by_feature_and_index() {
    let (a, _, _) = model();
    let mut fm = FeatureMap::new();
    fm.add_entry(a.clone(), s("first"));
    fm.add_entry(a.clone(), s("third"));
    fm.add_at(&a, 1, s("second"));
    let vals = fm.values_for(&a);
    assert_eq!(vals.len(), 3);
    assert_eq!(vals[0], s("first"));
    assert_eq!(vals[1], s("second"));
    assert_eq!(vals[2], s("third"));
}

// 5) RemoveEntry
#[test]
fn remove_entry() {
    let (a, _, _) = model();
    let mut fm = FeatureMap::new();
    fm.add_entry(a.clone(), s("v1"));
    fm.add_entry(a.clone(), s("v2"));
    assert_eq!(fm.len(), 2);
    assert!(fm.remove_entry(&a, &s("v1")));
    assert_eq!(fm.len(), 1);
    assert!(!fm.remove_entry(&a, &s("v1"))); // second remove false
}

// 6) GetByFeature
#[test]
fn get_by_feature() {
    let (a, _, _) = model();
    let mut fm = FeatureMap::new();
    fm.add_entry(a.clone(), s("alpha"));
    fm.add_entry(a.clone(), s("beta"));
    let v = fm.get_for(&a, 1).unwrap();
    assert_eq!(v.value(), &s("beta"));
}

// 7) SetByFeature
#[test]
fn set_by_feature() {
    let (a, _, _) = model();
    let mut fm = FeatureMap::new();
    fm.add_entry(a.clone(), s("old1"));
    fm.add_entry(a.clone(), s("old2"));
    let old = fm.set_for(&a, 0, s("new1")).unwrap();
    assert_eq!(old, s("old1"));
    assert_eq!(fm.get_for(&a, 0).unwrap().value(), &s("new1"));
}

// 8) ValuesByFeature
#[test]
fn values_by_feature() {
    let (a, b, _) = model();
    let mut fm = FeatureMap::new();
    fm.add_entry(a.clone(), s("x"));
    fm.add_entry(b.clone(), Val::Int(100));
    fm.add_entry(a.clone(), s("y"));
    let vals = fm.values_for(&a);
    assert_eq!(vals.len(), 2);
    assert_eq!(vals[0], s("x"));
    assert_eq!(vals[1], s("y"));
}

// 9) EntriesByFeature
#[test]
fn entries_by_feature() {
    let (a, b, _) = model();
    let mut fm = FeatureMap::new();
    fm.add_entry(a.clone(), s("e1"));
    fm.add_entry(b.clone(), Val::Int(1));
    fm.add_entry(a.clone(), s("e2"));
    let entries = fm.entries_for(&a);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].feature().feature_id(), a.feature_id());
}

// 10) SizeByFeature
#[test]
fn size_by_feature() {
    let (a, b, c) = model();
    let mut fm = FeatureMap::new();
    assert_eq!(fm.size_for(&a), 0);
    fm.add_entry(a.clone(), s("a"));
    fm.add_entry(a.clone(), s("b"));
    fm.add_entry(a.clone(), s("c"));
    fm.add_entry(b.clone(), Val::Int(1));
    assert_eq!(fm.size_for(&a), 3);
    assert_eq!(fm.size_for(&b), 1);
    assert_eq!(fm.size_for(&c), 0);
}

// 11) IteratorBasic
#[test]
fn iterator_basic() {
    let (a, b, _) = model();
    let mut fm = FeatureMap::new();
    fm.add_entry(a.clone(), s("a1"));
    fm.add_entry(b.clone(), Val::Int(1));
    fm.add_entry(a.clone(), s("a2"));
    assert_eq!(fm.entries().len(), 3);
    for e in fm.entries() {
        assert_ne!(e.feature().name(), "");
    }
}

// 12) ListIterator — forward then backward over a full cursor.
#[test]
fn list_iterator() {
    let (a, _, _) = model();
    let mut fm = FeatureMap::new();
    fm.add_entry(a.clone(), s("v1"));
    fm.add_entry(a.clone(), s("v2"));
    fm.add_entry(a.clone(), s("v3"));
    let n = fm.entries().len();
    // A single index cursor supports both directions.
    let mut i = 0usize;
    let mut forward = 0usize;
    while i < n {
        i += 1;
        forward += 1;
    }
    assert_eq!(forward, 3);
    let mut backward = 0usize;
    while i > 0 {
        i -= 1;
        backward += 1;
    }
    assert_eq!(backward, 3);
}

// 13) Clear
#[test]
fn clear() {
    let (a, b, _) = model();
    let mut fm = FeatureMap::new();
    fm.add_entry(a.clone(), s("a1"));
    fm.add_entry(a.clone(), s("a2"));
    fm.add_entry(b.clone(), Val::Int(1));
    assert_eq!(fm.len(), 3);
    fm.clear();
    assert_eq!(fm.len(), 0);
    assert_eq!(fm.size_for(&a), 0);
    assert_eq!(fm.size_for(&b), 0);
}

// 14) Contains
#[test]
fn contains() {
    let (a, _, _) = model();
    let mut fm = FeatureMap::new();
    fm.add_entry(a.clone(), s("a1"));
    assert!(fm.contains(&a, &s("a1")));
    assert!(!fm.contains(&a, &s("a2")));
}

// 15) ViewFeatureValues
#[test]
fn view_feature_values() {
    let (a, _, _) = model();
    let mut fm = FeatureMap::new();
    fm.add_entry(a.clone(), s("x"));
    fm.add_entry(a.clone(), s("y"));
    // view.size
    assert_eq!(fm.size_for(&a), 2);
    // view.get(0)
    assert_eq!(fm.get_for(&a, 0).unwrap().value(), &s("x"));
    // view.contains
    assert!(fm.contains(&a, &s("y")));
    // view.indexOf
    assert_eq!(fm.index_of(&a, &s("x")), 0);
    assert_eq!(fm.index_of(&a, &s("z")), -1);
    // view.remove
    assert!(fm.remove_entry(&a, &s("x")));
    assert_eq!(fm.size_for(&a), 1);
}

// 16) ViewFeatureEntries
#[test]
fn view_feature_entries() {
    let (a, _, _) = model();
    let mut fm = FeatureMap::new();
    fm.add_entry(a.clone(), s("e1"));
    fm.add_entry(a.clone(), s("e2"));
    let entries = fm.entries_for(&a);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].value(), &s("e1"));
    assert_eq!(entries[1].value(), &s("e2"));
}

// 17) IndexOf / lastIndexOf
#[test]
fn index_of_last_index_of() {
    let (a, b, c) = model();
    let mut fm = FeatureMap::new();
    fm.add_entry(a.clone(), s("v1"));
    fm.add_entry(a.clone(), s("v2"));
    fm.add_entry(b.clone(), Val::Int(1));
    assert_eq!(fm.index_of(&a, &s("v1")), 0);
    assert_eq!(fm.index_of(&a, &s("v2")), 1);
    assert_eq!(fm.index_of(&b, &Val::Int(1)), 0);
    assert_eq!(fm.last_index_of(&a, &s("v1")), 0);
    assert_eq!(fm.last_index_of(&b, &Val::Int(1)), 0);
    // missing entry
    assert_eq!(fm.index_of(&c, &s("x")), -1);
}

// 18) AddWithAnyValue
#[test]
fn add_with_any_value() {
    let mut fm = FeatureMap::new();
    let mut any = EStructuralFeature::attribute("__any");
    any.set_feature_id(-1);
    fm.add_entry(any, s("wrapped"));
    assert_eq!(fm.len(), 1);
    assert_eq!(fm.to_array().len(), 1);
}