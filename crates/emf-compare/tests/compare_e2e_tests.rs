//! Port of `CompareE2ETests.cpp` — end-to-end compare (match + diff) and merge
//! over two (or three) `Library -> [Book(title, pages)]` object graphs.
//!
//! The C++ suite loaded the `Ecore` metamodel from an XMI string and built
//! objects through the `EcoreFactory`. The Rust port builds the identical model
//! in memory with `DynamicEObject` via the shared `common` helper (see
//! `tests/common/mod.rs`); the object-graph shapes match the C++ scenarios.
//!
//! C++ -> Rust mappings used throughout:
//! - `compareModels(left, right)` (match + diff) -> `compare(Some(&l), Some(&r))`
//! - `compare(left, right, origin)` (3-way) -> `compare3(&l, &r, &o)`
//! - `compare(left, right, provider)` -> `compare_with_provider(l, r, provider)`
//! - `me.merge(comp, target)` (default RIGHT_TO_LEFT) ->
//!   `MergeEngine::merge(&mut comp, MergeDirection::RightToLeft)`
//! - `DiffKind::CHANGE/ADD/DELETE/MOVE` -> `DiffKind::Change/Add/Delete/Move`
//! - `ConflictKind::REAL/PSEUDO` -> `ConflictKind::Real/Pseudo`

mod common;

use common::{build_library, child_count, children, library_meta, read_str};
use emf_common::value::{ObjectRef, Val};
use emf_compare::comparison::{compare_with_provider, Comparison, ConflictKind};
use emf_compare::diff::{DiffKind, DiffType};
use emf_compare::merge_engine::{MergeDirection, MergeEngine};

/// Match + diff over two roots (C++ `compareModels`).
fn compare_models(left: &ObjectRef, right: &ObjectRef) -> Comparison {
    emf_compare::compare(Some(left), Some(right))
}

/// Count of diffs of a given `DiffKind` (C++ `countDiffs`).
fn count_diffs(comp: &Comparison, kind: DiffKind) -> usize {
    comp.differences().iter().filter(|d| d.kind() == kind).count()
}

/// Whether a `CHANGE` diff exists on a named attribute (C++ `hasChangeDiffOn`).
fn has_change_on(comp: &Comparison, attr: &str) -> bool {
    comp.differences().iter().any(|d| {
        d.kind() == DiffKind::Change && d.attribute_name() == attr
    })
}

// ===== Test 1: two identical models -> 0 diff =====
#[test]
fn compare_e2e_identical_models_no_diff() {
    let m = library_meta();
    let left = build_library(&m, "My Library", &[("B1".into(), 10), ("B2".into(), 20)]);
    let right = build_library(&m, "My Library", &[("B1".into(), 10), ("B2".into(), 20)]);
    let comp = compare_models(&left, &right);
    assert_eq!(comp.differences().len(), 0);
}

// ===== Test 2: attribute change -> per-attribute CHANGE diff =====
#[test]
fn compare_e2e_attribute_change_produces_per_attribute_diff() {
    let m = library_meta();
    let left = build_library(&m, "Old Name", &[("B1".into(), 10)]);
    let right = build_library(&m, "New Name", &[("B1".into(), 10)]);
    let comp = compare_models(&left, &right);
    assert!(comp.differences().len() > 0);
    assert!(has_change_on(&comp, "name"));
}

// ===== Test 3: added child -> ADD diff =====
#[test]
fn compare_e2e_added_child_produces_add_diff() {
    let m = library_meta();
    let left = build_library(&m, "Lib", &[("B1".into(), 10)]);
    let right = build_library(&m, "Lib", &[("B1".into(), 10), ("B2".into(), 20)]);
    let comp = compare_models(&left, &right);
    assert_eq!(count_diffs(&comp, DiffKind::Add), 1);
}

// ===== Test 4: removed child -> DELETE diff =====
#[test]
fn compare_e2e_removed_child_produces_delete_diff() {
    let m = library_meta();
    let left = build_library(&m, "Lib", &[("B1".into(), 10), ("B2".into(), 20)]);
    let right = build_library(&m, "Lib", &[("B1".into(), 10)]);
    let comp = compare_models(&left, &right);
    assert_eq!(count_diffs(&comp, DiffKind::Delete), 1);
}

// ===== Test 5: multi attribute change -> >=3 CHANGE diffs =====
#[test]
fn compare_e2e_multi_attribute_change_produces_multiple_diffs() {
    let m = library_meta();
    let left = build_library(&m, "L", &[("T1".into(), 10)]);
    let right = build_library(&m, "L_changed", &[("T1_changed".into(), 99)]);
    let comp = compare_models(&left, &right);
    assert!(count_diffs(&comp, DiffKind::Change) >= 3);
    assert!(has_change_on(&comp, "name"));
    assert!(has_change_on(&comp, "title"));
    assert!(has_change_on(&comp, "pages"));
}

// ===== Test 6: reordered children -> MOVE diff =====
#[test]
fn compare_e2e_reordered_children_produces_move_diff() {
    let m = library_meta();
    let left = build_library(&m, "Lib", &[("B1".into(), 1), ("B2".into(), 2), ("B3".into(), 3)]);
    let right = build_library(&m, "Lib", &[("B1".into(), 1), ("B3".into(), 3), ("B2".into(), 2)]);
    let comp = compare_models(&left, &right);
    assert!(count_diffs(&comp, DiffKind::Move) > 0);
}

// ===== Test 6b: LCS minimal MOVE set ([B1,B2,B3]->[B1,B3,B2] = 1 MOVE) =====
#[test]
fn compare_e2e_lcs_move_minimal_move_set() {
    let m = library_meta();
    let left = build_library(&m, "Lib", &[("B1".into(), 1), ("B2".into(), 2), ("B3".into(), 3)]);
    let right = build_library(&m, "Lib", &[("B1".into(), 1), ("B3".into(), 3), ("B2".into(), 2)]);
    let comp = compare_models(&left, &right);
    assert_eq!(count_diffs(&comp, DiffKind::Move), 1);
}

// ===== Test 6c: full reversal LCS -> 3 MOVE =====
#[test]
fn compare_e2e_lcs_move_full_reversal() {
    let m = library_meta();
    let left = build_library(
        &m,
        "Lib",
        &[("A".into(), 1), ("B".into(), 2), ("C".into(), 3), ("D".into(), 4)],
    );
    let right = build_library(
        &m,
        "Lib",
        &[("D".into(), 4), ("C".into(), 3), ("B".into(), 2), ("A".into(), 1)],
    );
    let comp = compare_models(&left, &right);
    assert_eq!(count_diffs(&comp, DiffKind::Move), 3);
}

// ===== Test 7: merge attribute CHANGE toward left =====
#[test]
fn compare_e2e_merge_attribute_change_applied_to_target() {
    let m = library_meta();
    let left = build_library(&m, "OldName", &[("B1".into(), 10)]);
    let right = build_library(&m, "NewName", &[("B1".into(), 10)]);
    let mut comp = compare_models(&left, &right);
    MergeEngine::merge(&mut comp, MergeDirection::RightToLeft);
    assert_eq!(read_str(&left, "name").as_deref(), Some("NewName"));
}

// ===== Test 8: merge ADD child toward left =====
#[test]
fn compare_e2e_merge_added_child_applied_to_target() {
    let m = library_meta();
    let left = build_library(&m, "Lib", &[("B1".into(), 10)]);
    let right = build_library(&m, "Lib", &[("B1".into(), 10), ("B2".into(), 20)]);
    let mut comp = compare_models(&left, &right);
    MergeEngine::merge(&mut comp, MergeDirection::RightToLeft);
    assert_eq!(child_count(&left), 2);
}

// ===== Test 9: merge DELETE child toward left =====
#[test]
fn compare_e2e_merge_deleted_child_applied_to_target() {
    let m = library_meta();
    let left = build_library(&m, "Lib", &[("B1".into(), 10), ("B2".into(), 20)]);
    let right = build_library(&m, "Lib", &[("B1".into(), 10)]);
    let mut comp = compare_models(&left, &right);
    MergeEngine::merge(&mut comp, MergeDirection::RightToLeft);
    assert_eq!(children(&left).len(), 1);
}

// ===== Test 10: 3-way REAL conflict (left/right change same feature differently) =====
#[test]
fn compare_e2e_three_way_real_conflict_detected() {
    let m = library_meta();
    let origin = build_library(&m, "O", &[("B1".into(), 10)]);
    let left = build_library(&m, "L", &[("B1".into(), 10)]);
    let right = build_library(&m, "R", &[("B1".into(), 10)]);
    let comp = emf_compare::compare3(Some(&left), Some(&right), Some(&origin));
    assert!(comp.is_three_way());
    assert!(comp.conflicts().len() > 0);
    let has_real = comp.conflicts().iter().any(|c| c.kind() == ConflictKind::Real);
    assert!(has_real);
}

// ===== Test 11: 3-way PSEUDO conflict (left/right change to same value) =====
#[test]
fn compare_e2e_three_way_pseudo_conflict_detected() {
    let m = library_meta();
    let origin = build_library(&m, "O", &[("B1".into(), 10)]);
    let left = build_library(&m, "Same", &[("B1".into(), 10)]);
    let right = build_library(&m, "Same", &[("B1".into(), 10)]);
    let comp = emf_compare::compare3(Some(&left), Some(&right), Some(&origin));
    let has_pseudo = comp.conflicts().iter().any(|c| c.kind() == ConflictKind::Pseudo);
    assert!(has_pseudo);
}

// ===== Test 12: 3-way no conflict (different features) =====
#[test]
fn compare_e2e_three_way_no_conflict_when_different_features() {
    let m = library_meta();
    let origin = build_library(&m, "O", &[("T1".into(), 10)]);
    let left = build_library(&m, "L", &[("T1".into(), 10)]);
    let right = build_library(&m, "O", &[("T1".into(), 99)]);
    let comp = emf_compare::compare3(Some(&left), Some(&right), Some(&origin));
    assert_eq!(comp.conflicts().len(), 0);
}

// ===== Test 13: identifier provider matches by id, not proximity =====
// The provider uses `pages` as the object id; two books with completely
// different `title` but the same `pages` must still be matched as one pair,
// producing an `ATTRIBUTE_CHANGE` on `title` afterwards.
#[test]
fn compare_e2e_identifier_provider_matches_by_id_not_proximity() {
    let m = library_meta();
    let left = build_library(&m, "L", &[("ProGit".into(), 42)]);
    let right = build_library(&m, "R", &[("Refactoring".into(), 42)]);
    let provider = |obj: &ObjectRef| -> String {
        if obj.borrow().e_class() != "Book" {
            return String::new();
        }
        match obj.borrow().e_get("pages") {
            Some(Val::Int(p)) => p.to_string(),
            _ => String::new(),
        }
    };
    let comp = compare_with_provider(Some(&left), Some(&right), provider);

    // A Book match exists with both sides present (not ABSENT).
    let has_book_match = comp.matches().iter().any(|mm| {
        mm.left().is_some() && mm.right().is_some() && {
            let l = mm.left().unwrap();
            l.borrow().e_class() == "Book"
        }
    });
    assert!(has_book_match);

    // title differs -> at least one ATTRIBUTE_CHANGE.
    let attr_change = comp
        .differences()
        .iter()
        .filter(|d| d.type_() == DiffType::AttributeChange)
        .count();
    assert!(attr_change >= 1);
}