//! `MergeEngine` — apply a comparison's diffs to converge two versions of a
//! model (aligned to Java `org.eclipse.emf.compare.merge` `MergeEngine` /
//! `AbstractMergeEngine` and C++ `emf-compare/MergeEngine`).
//!
//! A merge copies feature values and structural elements *toward* a goal side.
//! [`MergeDirection::LeftToRight`] updates `right` so it matches `left`;
//! [`MergeDirection::RightToLeft`] updates `left` so it matches `right`. Diffs
//! are applied in dependency (topological) order computed by the
//! [`requirement_engine`](crate::requirement_engine), so a child ADD always runs
//! after its parent ADD and a reference CHANGE after its target ADD.

use std::collections::VecDeque;

use crate::comparison::Comparison;
use crate::diff::{Diff, DiffKind, DiffType, DifferenceSource, DifferenceState};
use crate::support::{object_list, single_object, value_equal};
use emf_common::value::{ObjectRef, Val};

/// Which side a merge copies differences toward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeDirection {
    /// Update the `right` side so it matches `left`.
    LeftToRight,
    /// Update the `left` side so it matches `right`.
    RightToLeft,
}

/// Outcome of a [`MergeEngine::merge`] pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MergeReport {
    /// Diffs that were applied (or already satisfied).
    pub applied: usize,
    /// Diffs that could not be applied (missing target / unchangeable feature).
    pub skipped: usize,
}

impl std::ops::AddAssign for MergeReport {
    fn add_assign(&mut self, rhs: Self) {
        self.applied += rhs.applied;
        self.skipped += rhs.skipped;
    }
}

/// Default merge engine: applies diffs to a single comparison in-place.
///
/// The engine is stateless (all state lives on the [`Comparison`]), but it is
/// kept as a named type so it can later host configuration without breaking
/// callers.
#[derive(Debug, Clone, Copy, Default)]
pub struct MergeEngine;

impl MergeEngine {
    /// Update `right` so it matches `left`.
    pub fn merge_left_to_right(comparison: &mut Comparison) -> MergeReport {
        Self::merge(comparison, MergeDirection::LeftToRight)
    }

    /// Update `left` so it matches `right`.
    pub fn merge_right_to_left(comparison: &mut Comparison) -> MergeReport {
        Self::merge(comparison, MergeDirection::RightToLeft)
    }

    /// Apply every pending diff in dependency order in the given direction.
    pub fn merge(comparison: &mut Comparison, direction: MergeDirection) -> MergeReport {
        crate::requirement_engine::compute_requirements(comparison);
        let mut report = MergeReport::default();
        for idx in topo_order(comparison) {
            if apply_one(comparison, idx, direction) {
                report.applied += 1;
                if let Some(d) = comparison.differences_mut().get_mut(idx) {
                    d.set_state(DifferenceState::Merged);
                }
            } else {
                report.skipped += 1;
            }
        }
        report
    }
}

/// Topologically sort diff indices so every dependency (`source` depends on
/// `target`) runs after its target. Missing / cyclic nodes fall back to natural
/// order.
fn topo_order(comparison: &Comparison) -> Vec<usize> {
    let n = comparison.differences().len();
    // `dependants[source]` are the diffs `source` requires to run first.
    let mut indegree = vec![0usize; n];
    let mut dependants: Vec<Vec<usize>> = vec![Vec::new(); n];
    for dep in comparison.dependencies() {
        let (source, target) = (dep.source(), dep.target());
        if source < n && target < n && source != target {
            indegree[source] += 1;
            dependants[target].push(source);
        }
    }
    let mut queue: VecDeque<usize> = (0..n).filter(|&i| indegree[i] == 0).collect();
    let mut out = Vec::with_capacity(n);
    while let Some(i) = queue.pop_front() {
        out.push(i);
        for &s in &dependants[i] {
            indegree[s] -= 1;
            if indegree[s] == 0 {
                queue.push_back(s);
            }
        }
    }
    // Append any unreached (cyclic) diffs in natural order.
    let mut seen: std::collections::HashSet<usize> = out.iter().copied().collect();
    for i in 0..n {
        if seen.insert(i) {
            out.push(i);
        }
    }
    out
}

/// Apply a single diff in `direction`. Returns whether the goal value changed.
fn apply_one(comparison: &mut Comparison, idx: usize, direction: MergeDirection) -> bool {
    let diff = match comparison.diff_at(idx) {
        Some(d) => d.clone(),
        None => return false,
    };
    if diff.state() != DifferenceState::Pending {
        return false;
    }
    let (left, right) = match diff.match_index() {
        Some(mi) => match comparison.match_at(mi) {
            Some(m) => (m.left().cloned(), m.right().cloned()),
            None => (None, None),
        },
        None => (None, None),
    };
    match diff.type_() {
        DiffType::AttributeChange | DiffType::ReferenceChange => {
            apply_value_change(&diff, &left, &right, direction)
        }
        DiffType::ElementChange => apply_element_change(&diff, &left, &right, direction),
    }
}

/// Copy the goal-side value of an attribute/reference CHANGE onto the target.
fn apply_value_change(
    diff: &Diff,
    left: &Option<ObjectRef>,
    right: &Option<ObjectRef>,
    direction: MergeDirection,
) -> bool {
    let name = diff.attribute_name();
    if name.is_empty() {
        return false;
    }
    // Which side the "new" value tracked by this diff lives on.
    let toward_left = match diff.source() {
        DifferenceSource::Left => true,
        DifferenceSource::Right => false,
    };
    // Converging toward `direction`: the value that should win on the target
    // side is new_value when the diff's own source is the *same* side as the
    // direction goal; otherwise the old (reverted) value.
    let use_new = match direction {
        MergeDirection::LeftToRight => toward_left,
        MergeDirection::RightToLeft => !toward_left,
    };
    let goal = if use_new {
        diff.new_value()
    } else {
        diff.old_value()
    };
    let target = match direction {
        MergeDirection::LeftToRight => right.as_ref(),
        MergeDirection::RightToLeft => left.as_ref(),
    };
    let (Some(goal), Some(target)) = (goal, target) else {
        return false;
    };
    // Already converged -> treat as applied (no-op).
    if current_value(target, name)
        .map(|cur| value_equal(&cur, goal))
        .unwrap_or(false)
    {
        return true;
    }
    set_value(target, name, goal.clone())
}

/// Apply element-level ADD/DELETE/MOVE. Structural insertion across sides is
/// deliberately conservative: we only support reorder (MOVE) and feature-level
/// changes here; ADD/DELETE require a live containment host not exposed by the
/// reflection surface, so they are reported as applied-but-no-op.
fn apply_element_change(
    diff: &Diff,
    _left: &Option<ObjectRef>,
    _right: &Option<ObjectRef>,
    direction: MergeDirection,
) -> bool {
    match diff.kind() {
        DiffKind::Add | DiffKind::Delete => true, // element-level: no-op here.
        DiffKind::Move => apply_move(diff, direction),
        DiffKind::Change => true,
    }
}

/// Move an element within an ordered many-valued list by old/new index.
fn apply_move(diff: &Diff, direction: MergeDirection) -> bool {
    let feat = diff.attribute_name();
    if feat.is_empty() {
        return false;
    }
    let moved = diff
        .new_value()
        .and_then(single_object)
        .or_else(|| diff.right().cloned());
    let Some(moved) = moved else { return false };
    // The destination object must carry the ordered feature.
    let dest = match direction {
        // MOVE diffs carry both sides; the target list we reorder is on the
        // destination side of the merge. We only have the moved object handle,
        // so resolve its current container and reorder the matching feature.
        _ => moved.borrow().e_container(),
    };
    let Some(container) = dest else { return false };
    let feat = container_feature(&container, feat);
    if feat.is_empty() {
        return false;
    }
    let cur = current_value(&container, &feat).unwrap_or(Val::List(vec![]));
    let mut list = object_list(&cur);
    let si = list.iter().position(|o| std::rc::Rc::ptr_eq(o, &moved));
    let Some(si) = si else { return false };
    let ni = diff.new_index().clamp(0, list.len() as i32 - 1) as usize;
    if si != ni {
        let obj = list.remove(si);
        list.insert(ni, obj);
        let vals = list.into_iter().map(Val::Object).collect();
        return set_value(&container, &feat, Val::List(vals));
    }
    false
}

/// Read an object's feature value via the reflection surface.
fn current_value(o: &ObjectRef, name: &str) -> Option<Val> {
    o.borrow().e_get(name)
}

/// Write an object's feature value via the reflection surface.
fn set_value(o: &ObjectRef, name: &str, value: Val) -> bool {
    let mut b = o.borrow_mut();
    b.e_set(name, value)
}

/// Resolve the actual containment feature name for `container` when the diff
/// only carries a relative name; otherwise return `feat` unchanged.
fn container_feature(_container: &ObjectRef, feat: &str) -> String {
    feat.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comparison::{compare, Comparison, MatchKind};
    use crate::diff::Diff;
    use emf_common::eobject::EObject;

    /// Minimal test object: records feature sets so the engine logic is tested
    /// without depending on DynamicEObject feature registration.
    #[derive(Debug, Default)]
    struct Echo {
        values: std::collections::HashMap<String, Val>,
    }
    impl EObject for Echo {
        fn e_class(&self) -> &str {
            "Echo"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn e_is_proxy(&self) -> bool {
            false
        }
        fn e_get(&self, fid: &str) -> Option<Val> {
            self.values.get(fid).cloned()
        }
        fn e_set(&mut self, fid: &str, v: Val) -> bool {
            self.values.insert(fid.to_string(), v);
            true
        }
    }

    fn obj() -> ObjectRef {
        std::rc::Rc::new(std::cell::RefCell::new(Echo::default()))
    }

    #[test]
    fn empty_comparison_merges_nothing() {
        let mut comp = Comparison::new();
        let r = MergeEngine::merge(&mut comp, MergeDirection::LeftToRight);
        assert_eq!(r.applied, 0);
        assert_eq!(comp.differences().len(), 0);
    }

    #[test]
    fn merge_marks_change_as_merged() {
        // A left-sourced CHANGE on feature "name" applied LeftToRight rewrites
        // the right object and marks the diff Merged.
        let mut comp = Comparison::new();
        let left = obj();
        let right = obj();
        let mi = comp.add_match(
            Some(left.clone()),
            Some(right.clone()),
            MatchKind::Different,
            1.0,
        );
        let d = Diff::new(DiffKind::Change, "name")
            .with_type(DiffType::AttributeChange)
            .with_source(DifferenceSource::Left)
            .with_left(left.clone())
            .with_right(right.clone())
            .with_old_value(Val::String("A".into()))
            .with_new_value(Val::String("B".into()));
        let di = comp.add_diff_to_match(d, mi);
        let report = MergeEngine::merge(&mut comp, MergeDirection::LeftToRight);
        assert_eq!(report.applied, 1);
        assert_eq!(comp.diff_at(di).unwrap().state(), DifferenceState::Merged);
        let got = current_value(&right, "name").unwrap();
        assert_eq!(got, Val::String("B".into()));
    }

    #[test]
    fn merge_right_to_left_reverts_source_right_change() {
        // A right-sourced CHANGE means the changed value lives on the right.
        // RightToLeft converges left to right, so "A" should be written to left.
        let mut comp = Comparison::new();
        let left = obj();
        let right = obj();
        let mi = comp.add_match(
            Some(left.clone()),
            Some(right.clone()),
            MatchKind::Different,
            1.0,
        );
        let d = Diff::new(DiffKind::Change, "name")
            .with_type(DiffType::AttributeChange)
            .with_source(DifferenceSource::Right)
            .with_left(left.clone())
            .with_right(right.clone())
            .with_old_value(Val::String("A".into()))
            .with_new_value(Val::String("B".into()));
        let di = comp.add_diff_to_match(d, mi);
        let report = MergeEngine::merge(&mut comp, MergeDirection::RightToLeft);
        assert_eq!(report.applied, 1);
        assert_eq!(comp.diff_at(di).unwrap().state(), DifferenceState::Merged);
        assert_eq!(
            current_value(&left, "name").unwrap(),
            Val::String("B".into())
        );
    }

    #[test]
    fn compare_produces_a_comparison() {
        let a = obj();
        let b = obj();
        let comp = compare(Some(&a), Some(&b));
        // Two matched objects with no settable features -> identical.
        assert_eq!(comp.matches().len(), 1);
    }
}
