//! Port of `CompareP0RegressionTests.cpp` — P0 regression coverage.
//!
//! The C++ suite loaded three `Ecore` metamodels from XMI strings (Shop/Item,
//! Library/Book/Author, Parent/Child) and built objects through the factory.
//! The Rust port builds identical in-memory `DynamicEObject` graphs via the
//! shared `common` helper — see `tests/common/mod.rs` for the model shapes.
//!
//! C++ -> Rust mappings:
//! - `compareModels` (match + diff) -> `compare(Some(&l), Some(&r))`
//! - `compare(left, right, origin)` -> `compare3(&l, &r, &o)`
//! - `me.setUseIdAttribute(false); me.match(...); DiffEngine.de.diff(...)` ->
//!   `me.set_use_id_attribute(false); me.match_2way(...); diff_engine::do_diff(...)`
//! - `MergeEngine::merge(comp, target[, direction])` ->
//!   `MergeEngine::merge(&mut comp, MergeDirection::…)`
//! - `RequirementEngine::computeRequirements(comp)` -> `requirement_engine::compute_requirements(&mut comp)`
//! - `DiffKind`/`DiffType`/`ConflictKind` map to their lower-case Rust variants.

mod common;

use common::{
    bidir_meta, build_lib_author, build_lib_author_bob, build_parent, build_shop, lib_author_meta,
    read_str, ref_list, shop_meta,
};
use emf_common::value::ObjectRef;
use emf_compare::comparison::{Comparison, ConflictKind};
use emf_compare::diff::{DiffKind, DiffType};
use emf_compare::match_engine::MatchEngine;
use emf_compare::merge_engine::{MergeDirection, MergeEngine};
use std::rc::Rc;

/// Match + diff over two roots (C++ `compareModels`).
fn compare_models(left: &ObjectRef, right: &ObjectRef) -> Comparison {
    emf_compare::compare(Some(left), Some(right))
}

/// Count of diffs of a given `DiffKind` (C++ `countDiffs`).
fn count_diffs(comp: &Comparison, kind: DiffKind) -> usize {
    comp.differences().iter().filter(|d| d.kind() == kind).count()
}

// ===== P0-1: 自动 ID 匹配 =====

/// Same ID + same class but different attributes still match (no ADD/DELETE),
/// and the differing attribute yields one CHANGE.
#[test]
fn compare_p0_auto_id_match_same_id_different_attr_matches_as_different() {
    let m = shop_meta();
    let left = build_shop(&m, &[("A".into(), "foo".into())]);
    let right = build_shop(&m, &[("A".into(), "bar".into())]);
    let comp = compare_models(&left, &right);
    assert!(comp.matches().len() >= 2); // Shop match + Item match
    assert_eq!(count_diffs(&comp, DiffKind::Change), 1);
    assert_eq!(count_diffs(&comp, DiffKind::Add), 0);
    assert_eq!(count_diffs(&comp, DiffKind::Delete), 0);
}

/// Different insertion order but same IDs still pairs by ID → no CHANGE diff.
#[test]
fn compare_p0_auto_id_match_different_order_still_pairs_by_id() {
    let m = shop_meta();
    let left = build_shop(
        &m,
        &[("A".into(), "a".into()), ("B".into(), "b".into())],
    );
    let right = build_shop(
        &m,
        &[("B".into(), "b".into()), ("A".into(), "a".into())],
    );
    let comp = compare_models(&left, &right);
    assert_eq!(count_diffs(&comp, DiffKind::Change), 0);
    assert_eq!(count_diffs(&comp, DiffKind::Add), 0);
    assert_eq!(count_diffs(&comp, DiffKind::Delete), 0);
}

/// Disabling the ID matcher falls back to proximity (still one CHANGE).
#[test]
fn compare_p0_auto_id_match_disabled_falls_back_to_proximity() {
    let m = shop_meta();
    let left = build_shop(&m, &[("A".into(), "a".into())]);
    let right = build_shop(&m, &[("A".into(), "b".into())]);
    let mut comp = Comparison::new();
    let mut me = MatchEngine::new();
    me.set_use_id_attribute(false);
    me.match_2way(Some(&left), Some(&right), &mut comp);
    emf_compare::diff_engine::do_diff(&mut comp);
    assert_eq!(count_diffs(&comp, DiffKind::Change), 1);
}

// ===== P0-2: Diff 子类型 + old/new value =====

/// A `name` CHANGE is `ATTRIBUTE_CHANGE` with old/new values "foo"/"bar".
#[test]
fn compare_p0_diff_type_attribute_change_set_on_attribute_diff() {
    let m = shop_meta();
    let left = build_shop(&m, &[("A".into(), "foo".into())]);
    let right = build_shop(&m, &[("A".into(), "bar".into())]);
    let comp = compare_models(&left, &right);
    let found = comp.differences().iter().any(|d| {
        if d.kind() == DiffKind::Change && d.attribute_name() == "name" {
            assert_eq!(d.type_(), DiffType::AttributeChange);
            assert!(d.old_value().is_some());
            assert!(d.new_value().is_some());
            assert_eq!(d.old_value().and_then(|v| v.as_str()), Some("foo"));
            assert_eq!(d.new_value().and_then(|v| v.as_str()), Some("bar"));
            true
        } else {
            false
        }
    });
    assert!(found);
}

/// An ADD is `ELEMENT_CHANGE` with a non-empty new value.
#[test]
fn compare_p0_diff_type_element_change_set_on_add_delete() {
    let m = shop_meta();
    let left = build_shop(&m, &[("A".into(), "a".into())]);
    let right = build_shop(&m, &[("A".into(), "a".into()), ("B".into(), "b".into())]);
    let comp = compare_models(&left, &right);
    let found_add = comp.differences().iter().any(|d| {
        if d.kind() == DiffKind::Add {
            assert_eq!(d.type_(), DiffType::ElementChange);
            assert!(d.new_value().is_some());
            true
        } else {
            false
        }
    });
    assert!(found_add);
}

// ===== P0-3: 多值 feature 冲突检测 =====

/// Both sides add different items (B vs C) → REAL conflict.
#[test]
fn compare_p0_multi_value_conflict_real_when_both_sides_add_different() {
    let m = shop_meta();
    let origin = build_shop(&m, &[("A".into(), "a".into())]);
    let left = build_shop(&m, &[("A".into(), "a".into()), ("B".into(), "b".into())]);
    let right = build_shop(&m, &[("A".into(), "a".into()), ("C".into(), "c".into())]);
    let comp = emf_compare::compare3(Some(&left), Some(&right), Some(&origin));
    assert!(comp.conflicts().len() > 0);
    let has_real = comp.conflicts().iter().any(|c| c.kind() == ConflictKind::Real);
    assert!(has_real);
}

/// Both sides add the same item (same id) → PSEUDO conflict.
#[test]
fn compare_p0_multi_value_conflict_pseudo_when_both_sides_add_same() {
    let m = shop_meta();
    let origin = build_shop(&m, &[("A".into(), "a".into())]);
    let left = build_shop(&m, &[("A".into(), "a".into()), ("B".into(), "b".into())]);
    let right = build_shop(&m, &[("A".into(), "a".into()), ("B".into(), "b".into())]);
    let comp = emf_compare::compare3(Some(&left), Some(&right), Some(&origin));
    let has_pseudo = comp.conflicts().iter().any(|c| c.kind() == ConflictKind::Pseudo);
    assert!(has_pseudo);
}

// ===== P0-4: MergeEngine 双向 =====

/// LEFT_TO_RIGHT syncs left's changes onto the right side.
#[test]
fn compare_p0_merge_left_to_right_applies_left_changes_to_right() {
    let m = shop_meta();
    let left = build_shop(&m, &[("A".into(), "new".into())]);
    let right = build_shop(&m, &[("A".into(), "old".into())]);
    let mut comp = compare_models(&left, &right);
    MergeEngine::merge(&mut comp, MergeDirection::LeftToRight);
    let right_items = ref_list(&right, "items");
    assert_eq!(right_items.len(), 1);
    assert_eq!(read_str(&right_items[0], "name").as_deref(), Some("new"));
}

/// Default RIGHT_TO_LEFT syncs right's changes onto the left side.
#[test]
fn compare_p0_merge_right_to_left_default_direction() {
    let m = shop_meta();
    let left = build_shop(&m, &[("A".into(), "old".into())]);
    let right = build_shop(&m, &[("A".into(), "new".into())]);
    let mut comp = compare_models(&left, &right);
    MergeEngine::merge(&mut comp, MergeDirection::RightToLeft);
    let left_items = ref_list(&left, "items");
    assert_eq!(left_items.len(), 1);
    assert_eq!(read_str(&left_items[0], "name").as_deref(), Some("new"));
}

// ===== P0-5: ADD merge 克隆语义 =====

/// The added object is cloned into the target side, never sharing the pointer.
#[test]
fn compare_p0_merge_add_clones_object_not_sharing_pointer() {
    let m = shop_meta();
    let left = build_shop(&m, &[("A".into(), "a".into())]);
    let right = build_shop(&m, &[("A".into(), "a".into()), ("B".into(), "b".into())]);
    let mut comp = compare_models(&left, &right);
    MergeEngine::merge(&mut comp, MergeDirection::RightToLeft);
    let left_items = ref_list(&left, "items");
    let right_items = ref_list(&right, "items");
    assert_eq!(left_items.len(), 2);
    let left_added = &left_items[1];
    let right_added = &right_items[1];
    assert!(!Rc::ptr_eq(left_added, right_added));
    assert_eq!(read_str(left_added, "name").as_deref(), Some("b"));
}

// ===== P0-6: CHANGE reference merge 映射 =====

/// Merging RIGHT_TO_LEFT maps the changed `Book.author` onto the left-side author.
#[test]
fn compare_p0_merge_change_reference_maps_to_target_side_object() {
    let m = lib_author_meta();
    let left = build_lib_author(&m, "Alice", "T");
    let right = build_lib_author(&m, "Bob", "T");
    let mut comp = compare_models(&left, &right);
    MergeEngine::merge(&mut comp, MergeDirection::RightToLeft);

    let left_books = ref_list(&left, "books");
    let left_authors = ref_list(&left, "authors");
    assert_eq!(left_books.len(), 1);
    assert_eq!(left_authors.len(), 1);
    let left_book = &left_books[0];
    let left_author = &left_authors[0];
    // The book's author now points at the (renamed) left-side author.
    let book_author = left_book
        .borrow()
        .e_get("author")
        .and_then(|v| v.as_object().cloned())
        .expect("book.author set");
    assert!(Rc::ptr_eq(&book_author, left_author));
    assert_eq!(read_str(&book_author, "name").as_deref(), Some("Bob"));
}

/// REFERENCE_CHANGE to a freshly added object must resolve to the clone (P0 G6).
#[test]
fn compare_p0_merge_reference_change_to_added_object_maps_to_clone() {
    let m = lib_author_meta();
    let left = build_lib_author_bob(&m, "T", false); // authors=[Alice], book.author=Alice
    let right = build_lib_author_bob(&m, "T2", true); // authors=[Alice, Bob], book.author=Bob
    let mut comp = compare_models(&left, &right);

    // ADD(Bob) and REFERENCE_CHANGE(Book.author) must both be present.
    assert!(count_diffs(&comp, DiffKind::Add) >= 1);
    let has_ref_change = comp
        .differences()
        .iter()
        .any(|d| d.type_() == DiffType::ReferenceChange);
    assert!(has_ref_change);

    // G6: a dependency (REFERENCE_CHANGE depends on ADD Bob) is computed.
    emf_compare::requirement_engine::compute_requirements(&mut comp);
    assert!(comp.dependencies().len() > 0);

    MergeEngine::merge(&mut comp, MergeDirection::RightToLeft);

    // left.authors now holds 2 (Alice + a cloned Bob).
    let left_authors = ref_list(&left, "authors");
    assert_eq!(left_authors.len(), 2);
    let left_bob = left_authors
        .iter()
        .find(|a| read_str(a, "name").as_deref() == Some("Bob"))
        .cloned()
        .expect("a Bob clone exists on the left");
    // The changed Book.author must point at the left-side Bob clone.
    let left_books = ref_list(&left, "books");
    assert_eq!(left_books.len(), 1);
    let book_author = left_books[0]
        .borrow()
        .e_get("author")
        .and_then(|v| v.as_object().cloned())
        .expect("book.author set");
    assert!(Rc::ptr_eq(&book_author, &left_bob));
}

// ===== P0-7: eOpposite 维护 =====

/// ADD merge keeps the bidirectional `Child.parent` back-reference in sync.
#[test]
fn compare_p0_merge_eopposite_maintained_on_add() {
    let m = bidir_meta();
    let left = build_parent(&m, "P", None);
    let right = build_parent(&m, "P", Some("C"));
    let mut comp = compare_models(&left, &right);
    MergeEngine::merge(&mut comp, MergeDirection::RightToLeft);

    let left_children = ref_list(&left, "children");
    assert_eq!(left_children.len(), 1);
    let left_child = &left_children[0];
    let parent_ref = left_child
        .borrow()
        .e_get("parent")
        .and_then(|v| v.as_object().cloned())
        .expect("child.parent set");
    assert!(Rc::ptr_eq(&parent_ref, &left));
}

// ===== P0-8: Equivalence =====

/// A non-containment `Book -> Author` reference yields an Equivalence in 2-way.
#[test]
fn compare_p0_equivalence_tracked_for_non_containment_reference() {
    let m = lib_author_meta();
    let lib = build_lib_author(&m, "Alice", "T");
    let comp = emf_compare::compare(Some(&lib), Some(&lib));
    assert!(comp.equivalences().len() > 0);
}

/// Equivalence also works in 3-way.
#[test]
fn compare_p0_equivalence_tracked_in_three_way_compare() {
    let m = lib_author_meta();
    let left = build_lib_author(&m, "Alice", "T");
    let right = build_lib_author(&m, "Alice", "T");
    let origin = build_lib_author(&m, "Alice", "T");
    let comp = emf_compare::compare3(Some(&left), Some(&right), Some(&origin));
    assert!(comp.equivalences().len() > 0);
}