//! Port of `ComparisonTests.cpp` — `Comparison` container behaviour.
//!
//! C++ uses `addMatch(left, right, kind, similarity) -> Match&` with raw
//! `nullptr` for the absent side; Rust models the same call as
//! `Comparison::add_match(left: Option<ObjectRef>, right: Option<ObjectRef>, ...)`
//! returning the index of the appended `Match` (indexed access via
//! `matches()` / `match_at(i)`, mirroring the C++ `getMatches()[i]`).

use emf_compare::comparison::{Comparison, MatchKind};

/// C++ `Comparison_AddMatch`: an `IDENTICAL`/similarity-1.0 match is appended.
#[test]
fn comparison_add_match() {
    let mut comp = Comparison::new();
    let _idx = comp.add_match(None, None, MatchKind::Identical, 1.0);
    assert_eq!(comp.matches().len(), 1);
    let m = comp.match_at(0).expect("match 0");
    assert_eq!(m.kind(), MatchKind::Identical);
    assert_eq!(m.similarity(), 1.0);
}

/// C++ `Comparison_Differences_Empty`: a fresh comparison holds no diffs.
#[test]
fn comparison_differences_empty() {
    let comp = Comparison::new();
    assert_eq!(comp.differences().len(), 0);
}

/// C++ `Comparison_Clear`: `clear()` empties every result collection.
#[test]
fn comparison_clear() {
    let mut comp = Comparison::new();
    comp.add_match(None, None, MatchKind::Identical, 1.0);
    comp.clear();
    assert_eq!(comp.matches().len(), 0);
}