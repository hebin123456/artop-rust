//! `DiffEngine` — per-`Match` feature differencing (aligned to Java
//! `org.eclipse.emf.compare.diff.DefaultDiffEngine`, C++
//! `emf-compare/DiffEngine`).
//!
//! Produces per-feature `CHANGE` diffs for `DIFFERENT` matches, whole-object
//! `ADD`/`DELETE` diffs for `ABSENT_*` matches, and minimal `MOVE` diffs for
//! ordered many-valued references (via LCS, with a naive fallback for very
//! large lists to bound memory). A `leftToRight` match map is built once at the
//! `diff` entry to avoid quadratic rebuilds.

use crate::comparison::{Comparison, MatchKind};
use crate::diff::{Diff, DiffKind, DiffType, DifferenceSource};
use crate::support::{all_features, is_same, key, object_list, single_object, value_equal};
use emf_common::value::{ObjectRef, Val};
use std::collections::HashMap;

/// LCS DP dimension cap (2048² int ≈ 16 MB), falling back to a naive scan.
const LCS_MAX_N: usize = 2048;

/// Run the full difference pass over a comparison.
///
/// Public so integration tests can drive the manual match→diff pipeline exactly
/// like C++ `DiffEngine::diff(comp)` (e.g. after configuring a `MatchEngine`).
pub fn do_diff(comp: &mut Comparison) {
    // Build left→right object mapping from matches.
    let mut left_to_right: HashMap<usize, ObjectRef> = HashMap::new();
    for m in comp.matches() {
        if let (Some(l), Some(r)) = (m.left(), m.right()) {
            left_to_right.insert(key(l), r.clone());
        }
    }
    // Collect the matches to process (indices stable across appends).
    let match_count = comp.matches().len();
    for idx in 0..match_count {
        diff_match(comp, idx, &left_to_right);
    }
}

/// Diff a single match (by index).
fn diff_match(comp: &mut Comparison, match_idx: usize, left_to_right: &HashMap<usize, ObjectRef>) {
    let (kind, left, right) = {
        let m = comp.match_at(match_idx).expect("match index");
        (m.kind(), m.left().cloned(), m.right().cloned())
    };

    match kind {
        MatchKind::Identical => {
            // Still detect moves in ordered many-valued references.
            if let (Some(l), Some(r)) = (&left, &right) {
                let features = all_features(l);
                detect_moves(comp, match_idx, l, r, &features, left_to_right);
            }
        }
        MatchKind::Different => {
            let Some(l) = left else { return };
            let Some(r) = right else { return };
            let features = all_features(&l);
            diff_attributes(comp, match_idx, &l, &r, &features);
            diff_single_value_references(comp, match_idx, &l, &r, &features, left_to_right);
            detect_moves(comp, match_idx, &l, &r, &features, left_to_right);
            // Degenerate: all features equal but match still DIFFERENT.
            if comp
                .match_at(match_idx)
                .map(|m| m.diff_indices().is_empty())
                .unwrap_or(false)
            {
                let d = Diff::new(DiffKind::Change, "")
                    .with_type(DiffType::ElementChange)
                    .with_left(l.clone())
                    .with_right(r.clone())
                    .with_source(DifferenceSource::Right)
                    .with_old_value(Val::Object(l))
                    .with_new_value(Val::Object(r));
                let (d, _) = attach(comp, match_idx, d);
                let _ = d;
            }
        }
        MatchKind::AbsentLeft => {
            // right exists, left missing -> ADD.
            let Some(r) = right else { return };
            let d = Diff::new(DiffKind::Add, "")
                .with_type(DiffType::ElementChange)
                .with_right(r.clone())
                .with_source(DifferenceSource::Right)
                .with_new_value(Val::Object(r));
            let _ = attach(comp, match_idx, d);
        }
        MatchKind::AbsentRight => {
            // left exists, right missing -> DELETE.
            let Some(l) = left else { return };
            let d = Diff::new(DiffKind::Delete, "")
                .with_type(DiffType::ElementChange)
                .with_left(l.clone())
                .with_source(DifferenceSource::Left)
                .with_old_value(Val::Object(l));
            let _ = attach(comp, match_idx, d);
        }
    }
}

/// Append a diff to the comparison and register it on its match.
fn attach(comp: &mut Comparison, match_idx: usize, mut d: Diff) -> (usize, Option<usize>) {
    d.set_match_index(Some(match_idx));
    let idx = comp.add_diff(d);
    if let Some(m) = comp.matches_mut().get_mut(match_idx) {
        m.push_diff(idx);
    }
    (idx, None)
}

/// Per-attribute CHANGE diffs.
fn diff_attributes(
    comp: &mut Comparison,
    midx: usize,
    left: &ObjectRef,
    right: &ObjectRef,
    features: &[emf_ecore::EStructuralFeature],
) {
    for sf in features {
        if sf.is_derived() || sf.is_transient() || !sf.is_changeable() || sf.is_reference() {
            continue;
        }
        let lv = e_get(left, sf.name());
        let rv = e_get(right, sf.name());
        if !value_equal(&lv, &rv) {
            let m = comp.match_at(midx).expect("match").left().cloned();
            let mr = comp.match_at(midx).expect("match").right().cloned();
            let d = Diff::new(DiffKind::Change, sf.name())
                .with_type(DiffType::AttributeChange)
                .with_left(m.unwrap_or_else(|| left.clone()))
                .with_right(mr.unwrap_or_else(|| right.clone()))
                .with_source(DifferenceSource::Right)
                .with_old_value(lv)
                .with_new_value(rv);
            let _ = attach(comp, midx, d);
        }
    }
}

/// Per-reference CHANGE diffs for single-valued, non-containment references.
fn diff_single_value_references(
    comp: &mut Comparison,
    midx: usize,
    left: &ObjectRef,
    right: &ObjectRef,
    features: &[emf_ecore::EStructuralFeature],
    left_to_right: &HashMap<usize, ObjectRef>,
) {
    for sf in features {
        if sf.is_derived() || sf.is_transient() || !sf.is_changeable() || !sf.is_reference() {
            continue;
        }
        if sf.is_many() || sf.is_containment() {
            continue;
        }
        let lv = e_get(left, sf.name());
        let rv = e_get(right, sf.name());
        if value_equal(&lv, &rv) {
            continue;
        }
        // Non-proxy cross-object references: treat same-match targets as equal.
        if let (Some(lo), Some(ro)) = (single_object(&lv), single_object(&rv)) {
            if !lo.borrow().e_is_proxy() && !ro.borrow().e_is_proxy() {
                if let Some(mapped) = left_to_right.get(&key(&lo)) {
                    if is_same(mapped, &ro) {
                        continue;
                    }
                }
            }
        }
        let m_l = comp.match_at(midx).expect("match").left().cloned();
        let m_r = comp.match_at(midx).expect("match").right().cloned();
        let d = Diff::new(DiffKind::Change, sf.name())
            .with_type(DiffType::ReferenceChange)
            .with_left(m_l.unwrap_or_else(|| left.clone()))
            .with_right(m_r.unwrap_or_else(|| right.clone()))
            .with_source(DifferenceSource::Right)
            .with_old_value(lv)
            .with_new_value(rv);
        let _ = attach(comp, midx, d);
    }
}

/// Detect minimal MOVE diffs in ordered many-valued references.
fn detect_moves(
    comp: &mut Comparison,
    midx: usize,
    left: &ObjectRef,
    right: &ObjectRef,
    features: &[emf_ecore::EStructuralFeature],
    left_to_right: &HashMap<usize, ObjectRef>,
) {
    for sf in features {
        if sf.is_derived() || sf.is_transient() || !sf.is_changeable() || !sf.is_reference() {
            continue;
        }
        if !sf.is_many() || !sf.is_ordered() {
            continue;
        }
        let left_list = object_list(&e_get(left, sf.name()));
        let right_list = object_list(&e_get(right, sf.name()));
        if left_list.len() != right_list.len() || left_list.is_empty() {
            continue;
        }
        let n = left_list.len();

        // right object -> index.
        let mut right_index: HashMap<usize, usize> = HashMap::new();
        for (k, o) in right_list.iter().enumerate() {
            right_index.insert(key(o), k);
        }

        let matched = |i: usize, j: usize| -> bool {
            let lo = &left_list[i];
            match left_to_right.get(&key(lo)) {
                Some(mapped) => is_same(mapped, &right_list[j]),
                None => false,
            }
        };

        // LCS marking of anchored (left) indices.
        let mut in_lcs_left = vec![false; n];
        if n <= LCS_MAX_N {
            // LCS DP with the Hirschberg-ish trace (O(n²) space acceptable here).
            let mut dp = vec![vec![0usize; n + 1]; n + 1];
            for i in 1..=n {
                for j in 1..=n {
                    dp[i][j] = if matched(i - 1, j - 1) {
                        dp[i - 1][j - 1] + 1
                    } else {
                        dp[i - 1][j].max(dp[i][j - 1])
                    };
                }
            }
            let (mut i, mut j) = (n, n);
            while i > 0 && j > 0 {
                if matched(i - 1, j - 1) && dp[i][j] == dp[i - 1][j - 1] + 1 {
                    in_lcs_left[i - 1] = true;
                    i -= 1;
                    j -= 1;
                } else if dp[i - 1][j] >= dp[i][j - 1] {
                    i -= 1;
                } else {
                    j -= 1;
                }
            }
        }

        // Non-anchored elements present on both sides -> MOVE.
        for old_idx in 0..n {
            if in_lcs_left[old_idx] {
                continue;
            }
            let lo = &left_list[old_idx];
            let Some(mapped) = left_to_right.get(&key(lo)) else {
                continue;
            };
            let Some(&new_idx) = right_index.get(&key(mapped)) else {
                continue;
            };
            if n <= LCS_MAX_N || old_idx as i32 != new_idx as i32 {
                let d = Diff::new(DiffKind::Move, sf.name())
                    .with_type(DiffType::ReferenceChange)
                    .with_left(lo.clone())
                    .with_right(mapped.clone())
                    .with_source(DifferenceSource::Right)
                    .with_old_index(old_idx as i32)
                    .with_new_index(new_idx as i32)
                    .with_old_value(Val::Object(lo.clone()))
                    .with_new_value(Val::Object(mapped.clone()));
                let _ = attach(comp, midx, d);
            }
        }
    }
}

/// Reflective read of a feature by name.
fn e_get(o: &ObjectRef, name: &str) -> Val {
    o.borrow().e_get(name).unwrap_or(Val::Null)
}
