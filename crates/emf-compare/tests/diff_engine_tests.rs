//! Port of `DiffEngineTests.cpp` — `DiffEngine` behaviour.
//!
//! Rust API mapping:
//! - `de.diff(comp)` -> `emf_compare::diff_engine::do_diff(&mut comp)`
//! - `addMatch(nullptr, nullptr, kind, similarity)` -> `add_match(None, None, kind, similarity)`
//!
//! The C++ `ProducesDiff_OnDifferent` test seeded a `DIFFERENT` match with two
//! `nullptr` objects and expected one `CHANGE`. The Rust `diff_engine` needs a
//! real left/right pair to inspect features (a `DIFFERENT` match with no object
//! side is a no-op). We therefore build two `Book`s with a differing `title` to
//! drive the same `DiffKind::CHANGE` outcome.

mod common;

use common::{build_library, children, library_meta};
use emf_compare::comparison::{Comparison, MatchKind};
use emf_compare::diff::{DiffKind, DiffType};

/// C++ `DiffEngine_NoDiff_OnIdentical`: an `IDENTICAL` match yields no diff.
#[test]
fn diff_engine_no_diff_on_identical() {
    let mut comp = Comparison::new();
    comp.add_match(None, None, MatchKind::Identical, 1.0);
    emf_compare::diff_engine::do_diff(&mut comp);
    assert_eq!(comp.differences().len(), 0);
}

/// C++ `DiffEngine_ProducesDiff_OnDifferent`: a `DIFFERENT` match yields a
/// `DiffKind::CHANGE` (here: `title` `AttributeChange` between two books).
#[test]
fn diff_engine_produces_diff_on_different() {
    let m = library_meta();
    let left_lib = build_library(&m, "L", &[("T1".to_string(), 10)]);
    let right_lib = build_library(&m, "L", &[("T2".to_string(), 20)]);
    let left_book = children(&left_lib)[0].clone();
    let right_book = children(&right_lib)[0].clone();

    let mut comp = Comparison::new();
    comp.add_match(
        Some(left_book),
        Some(right_book),
        MatchKind::Different,
        0.0,
    );
    emf_compare::diff_engine::do_diff(&mut comp);

    assert!(comp.differences().len() >= 1);
    // The first produced diff must be a CHANGE (title differs between the books).
    let d = comp.diff_at(0).expect("first diff");
    assert_eq!(d.kind(), DiffKind::Change);
    // It is an attribute-level change on "title".
    assert_eq!(d.type_(), DiffType::AttributeChange);
    assert_eq!(d.attribute_name(), "title");
}