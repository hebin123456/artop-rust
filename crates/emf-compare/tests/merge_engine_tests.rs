//! Port of `MergeEngineTests.cpp` — `MergeEngine` entry behaviour.
//!
//! Rust API mapping: `me.merge(comp, target)` (C++ where `target` is a nullable
//! root) becomes `MergeEngine::merge(&mut comp, MergeDirection::RightToLeft)`
//! (the C++ default direction). Rust `merge` returns a `MergeReport` rather than
//! a `bool`; a comparison with no applicable diffs yields `applied == 0`,
//! mirroring the C++ "returns false" (nothing to merge toward a null target).

use emf_compare::comparison::{Comparison, MatchKind};
use emf_compare::merge_engine::{MergeDirection, MergeEngine};

/// C++ `MergeEngine_NullTarget_ReturnsFalse`: nothing is applied when merging
/// toward a target that cannot be resolved (here: a comparison that has no
/// real diffs / no reachable target objects).
#[test]
fn merge_engine_null_target_returns_none() {
    let mut comp = Comparison::new();
    // A null-only IDENTICAL match carries no diff the merge engine can apply.
    comp.add_match(None, None, MatchKind::Identical, 1.0);
    let report = MergeEngine::merge(&mut comp, MergeDirection::RightToLeft);
    assert_eq!(report.applied, 0);
    assert_eq!(report.skipped, 0);
}