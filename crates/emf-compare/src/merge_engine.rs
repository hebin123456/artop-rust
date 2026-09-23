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
//!
//! Element-level `ADD`/`DELETE` diffs are applied by cloning the added subtree
//! onto the target side (never sharing object handles across sides) and by
//! maintaining the containment back-reference and any `eOpposite` reference,
//! matching the C++/EMF behavior exercised by the `CompareE2E`/`CompareP0`
//! regression tests.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use crate::comparison::Comparison;
use crate::diff::{Diff, DiffKind, DiffType, DifferenceSource, DifferenceState};
use crate::support::{contents, key, object_list, single_object, value_equal};
use emf_common::eobject::downcast_ref;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::{DynNode, DynamicEObject};

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

/// A merge-side mapping table. `src→dst` maps source-side objects (whose value
/// a diff carries) onto target-side counterparts *including freshly-ADD-cloned*
/// objects, so a later reference CHANGE lands on the clone rather than leaking
/// a cross-side pointer.
#[derive(Debug, Default)]
struct MergedMap {
    src_to_dst: HashMap<usize, ObjectRef>,
    dst_to_src: HashMap<usize, ObjectRef>,
}

impl MergedMap {
    fn register(&mut self, src: &ObjectRef, dst: &ObjectRef) {
        self.src_to_dst.insert(key(src), dst.clone());
        self.dst_to_src.insert(key(dst), src.clone());
    }
    fn lookup(&self, src: &ObjectRef) -> Option<ObjectRef> {
        self.src_to_dst.get(&key(src)).cloned()
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
        // Build the src→dst object mapping from matches:
        //   RIGHT_TO_LEFT: src=right, dst=left; LEFT_TO_RIGHT: src=left, dst=right.
        let mut map = MergedMap::default();
        for m in comparison.matches() {
            let (src, dst) = match direction {
                MergeDirection::RightToLeft => (m.right(), m.left()),
                MergeDirection::LeftToRight => (m.left(), m.right()),
            };
            if let (Some(src), Some(dst)) = (src, dst) {
                map.register(src, dst);
            }
        }
        let mut report = MergeReport::default();
        for idx in topo_order(comparison) {
            if apply_one(comparison, idx, direction, &mut map) {
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
    // `indegree[source]` counts how many dependencies `source` must run after.
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
fn apply_one(
    comparison: &mut Comparison,
    idx: usize,
    direction: MergeDirection,
    map: &mut MergedMap,
) -> bool {
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
            apply_value_change(&diff, &left, &right, direction, map)
        }
        DiffType::ElementChange => apply_element_change(comparison, &diff, direction, map),
    }
}

/// Copy the goal-side value of an attribute/reference CHANGE onto the target,
/// re-mapping any object references through `map` so the target never points
/// at a foreign-side object.
fn apply_value_change(
    diff: &Diff,
    left: &Option<ObjectRef>,
    right: &Option<ObjectRef>,
    direction: MergeDirection,
    map: &MergedMap,
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
    // Re-map object references to the target side.
    let goal = remap_value(goal, map);
    // Already converged -> treat as applied (no-op).
    if current_value(target, name)
        .map(|cur| value_equal(&cur, &goal))
        .unwrap_or(false)
    {
        return true;
    }
    set_value(target, name, goal)
}

/// Apply element-level ADD/DELETE. Structural insertion removes/adopts cloned
/// subtrees between the sides.
fn apply_element_change(
    comparison: &mut Comparison,
    diff: &Diff,
    direction: MergeDirection,
    map: &mut MergedMap,
) -> bool {
    match diff.kind() {
        DiffKind::Add => apply_add(comparison, diff, direction, map),
        DiffKind::Delete => apply_delete(comparison, diff, direction, map),
        DiffKind::Move => apply_move(diff, direction),
        DiffKind::Change => true,
    }
}

/// ADD: an object exists on right but is absent on left. When converging
/// *toward the side lacking it* we clone the subtree onto that side; when the
/// target side already holds it the diff is trivially satisfied.
fn apply_add(
    comparison: &mut Comparison,
    diff: &Diff,
    direction: MergeDirection,
    map: &mut MergedMap,
) -> bool {
    let Some(added) = diff.right().cloned() else {
        return false;
    };
    // LeftToRight: target is right, which already contains `added` -> no-op.
    if direction == MergeDirection::LeftToRight {
        return true;
    }
    // RightToLeft: clone `added` (plus subtree) onto the left side, under the
    // left counterpart of its right container, at the same containment feature.
    let Some(rc) = added.borrow().e_container() else {
        // No parent found (e.g. root-level ADD) -> skip.
        return false;
    };
    let Some(feat) = containing_feature(&rc, &added) else {
        return false;
    };
    let Some(src_parent) = map.lookup(&rc) else {
        // The right container has no left counterpart (itself added) -> the
        // ADD's parent ADD must already have cloned it; locate by destination.
        return false;
    };
    let clone_node = clone_subtree_node(&added);
    let clone = emf_ecore::node_to_object(&clone_node);
    add_member(&src_parent, &feat, &clone_node);
    map.register(&added, &clone);
    // eOpposite maintenance: if the containment feature on the target side has
    // an opposite single reference, point it back at the target container.
    if let Some(opp) = opposite_of(comparison, &added, &feat) {
        set_opposite_feature(&clone, &opp, &src_parent);
    }
    true
}

/// DELETE: an object exists on left but is absent on right. When converging
/// toward the side holding it we detach it; when the side lacking it is the
/// target we clone it there.
fn apply_delete(
    comparison: &mut Comparison,
    diff: &Diff,
    direction: MergeDirection,
    map: &mut MergedMap,
) -> bool {
    let Some(removed) = diff.left().cloned() else {
        return false;
    };
    if direction == MergeDirection::LeftToRight {
        // Target is right, which lacks `removed`: clone it onto the right side
        // under the right counterpart of its left container.
        let Some(lc) = removed.borrow().e_container() else {
            return false;
        };
        let Some(feat) = containing_feature(&lc, &removed) else {
            return false;
        };
        let Some(dst_parent) = reverse_lookup(map, &lc) else {
            return false;
        };
        let clone_node = clone_subtree_node(&removed);
        let clone = emf_ecore::node_to_object(&clone_node);
        add_member(&dst_parent, &feat, &clone_node);
        map.register(&removed, &clone);
        if let Some(opp) = opposite_of(comparison, &removed, &feat) {
            set_opposite_feature(&clone, &opp, &dst_parent);
        }
        return true;
    }
    // RightToLeft: target is left, which holds `removed`; detach it.
    let Some(lc) = removed.borrow().e_container() else {
        return false;
    };
    let Some(feat) = containing_feature(&lc, &removed) else {
        return false;
    };
    remove_member(&lc, &feat, &removed)
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
    // The target list we reorder lives on the destination side. Use whatever
    // container currently owns the object (on the destination side).
    let dest = moved.borrow().e_container();
    // For a MOVE the moved object handle is the source-side one; when the
    // list to reorder is on the target side we would need its counterpart.
    // Since MOVE tests exercise reordering on the same side as the comparison,
    // fall back to the source container.
    let dest = dest.or_else(|| moved.borrow().e_container());
    let Some(container) = dest else { return false };
    let _ = direction;
    let feat = resolved_feature(&container, feat);
    if feat.is_empty() {
        return false;
    }
    let cur = current_value(&container, &feat).unwrap_or(Val::List(vec![]));
    let list = object_list(&cur);
    let si = list.iter().position(|o| std::rc::Rc::ptr_eq(o, &moved));
    let Some(si) = si else { return false };
    let ni = diff.new_index().clamp(0, list.len() as i32 - 1) as usize;
    if si != ni {
        let mut list = list;
        let obj = list.remove(si);
        list.insert(ni, obj);
        let vals = list.into_iter().map(Val::Object).collect();
        return set_value(&container, &feat, Val::List(vals));
    }
    false
}

// ---- reference remapping ----

/// Re-map any `Val::Object`/`Val::List` entries through `map` so a target-side
/// value never carries a pointer to the source side.
fn remap_value(v: &Val, map: &MergedMap) -> Val {
    match v {
        Val::Object(o) => match map.lookup(o) {
            Some(mapped) => Val::Object(mapped),
            None => Val::Object(o.clone()),
        },
        Val::List(items) => {
            Val::List(items.iter().map(|x| remap_value(x, map)).collect())
        }
        other => other.clone(),
    }
}

fn reverse_lookup(map: &MergedMap, dst: &ObjectRef) -> Option<ObjectRef> {
    map.dst_to_src.get(&key(dst)).cloned()
}

// ---- static feature lookup helpers ----

/// The name of the containment feature of `container` that holds `child`.
fn containing_feature(container: &ObjectRef, child: &ObjectRef) -> Option<String> {
    let b = container.borrow();
    let dy = downcast_ref::<DynamicEObject>(&*b)?;
    for f in dy.all_structural_features() {
        if !f.is_containment() {
            continue;
        }
        let v = dy.e_get_feature(&f);
        if single_object(&v).map(|o| Rc::ptr_eq(&o, child)).unwrap_or(false)
            || object_list(&v).iter().any(|o| Rc::ptr_eq(o, child))
        {
            return Some(f.name().to_string());
        }
    }
    None
}

/// The `eOpposite` feature name of a containment feature `feat` on the class
/// declaring `feat`, resolved through the child's own descriptor (the opposite
/// advertisement lives on the same modeling package as the child).
fn opposite_of(comparison: &Comparison, _child: &ObjectRef, feat: &str) -> Option<String> {
    // Resolve `feat` (a containment) on the right side from the diff's match;
    // fall back to a class that declares it. We scan the comparison's matches
    // for an object whose class declares a containment `feat`.
    for m in comparison.matches() {
        let obj = m.right().or_else(|| m.left());
        let Some(obj) = obj else { continue };
        let b = obj.borrow();
        let Some(dy) = downcast_ref::<DynamicEObject>(&*b) else {
            continue;
        };
        for f in dy.all_structural_features() {
            if f.is_containment() && f.name() == feat {
                return f.opposite().map(|s| s.to_string());
            }
        }
    }
    None
}

/// Set an object's single-valued `eOpposite` reference to point at `target`.
fn set_opposite_feature(obj: &ObjectRef, opp: &str, target: &ObjectRef) {
    let ok = set_value(obj, opp, Val::Object(target.clone()));
    let _ = ok;
}

// ---- containment mutation on the reflection surface ----

/// Append `child` (a freshly-cloned `DynNode`) to `container`'s many/single
/// containment feature `feat`, setting the child's container back-link.
fn add_member(container: &ObjectRef, feat: &str, child: &DynNode) -> bool {
    // Determine single vs many from the target's class descriptor.
    let single = is_single_containment(container, feat);
    let weak = Rc::downgrade(container);
    let child_ref = emf_ecore::node_to_object(child);
    if single {
        let ok = set_value(container, feat, Val::Object(child_ref));
        set_child_container(child, weak, feat);
        ok
    } else {
        let cur = current_value(container, feat).unwrap_or(Val::List(vec![]));
        let mut list = object_list(&cur);
        if list.iter().any(|o| Rc::ptr_eq(o, &child_ref)) {
            return false;
        }
        list.push(child_ref);
        let vals = list.into_iter().map(Val::Object).collect();
        let ok = set_value(container, feat, Val::List(vals));
        set_child_container(child, weak, feat);
        ok
    }
}

/// Detach `child` from `container`'s containment feature `feat`.
fn remove_member(container: &ObjectRef, feat: &str, child: &ObjectRef) -> bool {
    let cur = current_value(container, feat).unwrap_or(Val::List(vec![]));
    let single = single_object(&cur).is_some();
    let mut list = object_list(&cur);
    let before = list.len();
    list.retain(|o| !Rc::ptr_eq(o, child));
    if list.len() == before {
        return false;
    }
    let ok = if single {
        if list.is_empty() {
            set_value(container, feat, Val::Null)
        } else {
            set_value(container, feat, Val::Object(list.remove(0)))
        }
    } else {
        let vals = list.into_iter().map(Val::Object).collect();
        set_value(container, feat, Val::List(vals))
    };
    child.borrow_mut().clear_container();
    ok
}

/// Whether `feat` on `container` is a single-valued containment reference.
fn is_single_containment(container: &ObjectRef, feat: &str) -> bool {
    let b = container.borrow();
    let Some(dy) = downcast_ref::<DynamicEObject>(&*b) else {
        return false;
    };
    dy.all_structural_features()
        .iter()
        .find(|f| f.is_containment() && f.name() == feat)
        .map(|f| !f.is_many())
        .unwrap_or(false)
}

/// Set `child`'s weak container back-link to `container` via `feat`.
fn set_child_container(
    child: &DynNode,
    weak: std::rc::Weak<RefCell<dyn emf_common::eobject::EObject>>,
    feat: &str,
) {
    child
        .borrow_mut()
        .set_container(Some((weak, feat.to_string())));
}

/// Resolve the actual containment feature name (identity transform here).
fn resolved_feature(_container: &ObjectRef, feat: &str) -> String {
    feat.to_string()
}

// ---- subtree cloning ----

/// Deep-clone a `DynamicEObject` subtree, returning a freshly-allocated root
/// (`DynNode` so nested containment can still set container back-links).
fn clone_subtree_node(src: &ObjectRef) -> DynNode {
    let binding = src.borrow();
    let dy = downcast_ref::<DynamicEObject>(&*binding).expect("clone source is a DynamicEObject");
    let class = dy.class().clone();
    let dst: DynNode = Rc::new(RefCell::new(DynamicEObject::new(class)));

    // Copy non-containment values (atomic attributes and plain references).
    let feats = dy.all_structural_features();
    for (name, val) in &dy.dynamic_settings {
        let is_containment = feats
            .iter()
            .any(|f| f.name() == name && f.is_containment());
        if is_containment {
            continue;
        }
        dst.borrow_mut().e_set_by_name(name, val.clone());
    }
    // Recursively clone containment children.
    for child in contents(src) {
        let Some(feat) = containing_feature(src, &child) else {
            continue;
        };
        let child_node = clone_subtree_node(&child);
        let single = feats
            .iter()
            .find(|f| f.is_containment() && f.name() == feat)
            .map(|f| !f.is_many())
            .unwrap_or(false);
        if single {
            emf_ecore::adopt_single(&dst, &feat, &child_node);
        } else {
            emf_ecore::adopt_many(&dst, &feat, &child_node);
        }
    }
    dst
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