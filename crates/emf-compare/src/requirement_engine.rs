//! `RequirementEngine` — cross-`Diff` ordering dependencies (aligned to Java
//! `org.eclipse.emf.compare.internal.RequirementEngine`, C++
//! `emf-compare/RequirementEngine`).
//!
//! Computes `Dependency(source, target)` edges so the merge engine can apply
//! diffs in topological order (a parent ADD before its children ADD, an ADD
//! before the reference change pointing at it, etc.)

use crate::comparison::{Comparison, Dependency};
use crate::diff::{DiffKind, DiffType};
use crate::support::{key, single_object};
use emf_common::value::ObjectRef;
use std::collections::HashMap;

/// Compute dependency edges over a comparison's diffs.
///
/// Public so integration tests can assert dependency edges exist (the P0 G6
/// case) before handing the comparison to the merge engine, mirroring C++
/// `RequirementEngine::computeRequirements(comp)`.
pub fn compute_requirements(comp: &mut Comparison) {
    comp.dependencies_mut().clear();
    let mut seen: Vec<(usize, usize)> = Vec::new();

    // Index ADD/DELETE ELEMENT_CHANGE diffs by object.
    let mut add_diff_by_obj: HashMap<usize, usize> = HashMap::new();
    let mut del_diff_by_obj: HashMap<usize, usize> = HashMap::new();
    for (i, d) in comp.differences().iter().enumerate() {
        if d.type_() != DiffType::ElementChange {
            continue;
        }
        match d.kind() {
            DiffKind::Add => {
                if let Some(r) = d.right() {
                    add_diff_by_obj.insert(key(r), i);
                }
            }
            DiffKind::Delete => {
                if let Some(l) = d.left() {
                    del_diff_by_obj.insert(key(l), i);
                }
            }
            _ => {}
        }
    }

    // child -> container maps for left/right.
    let mut left_container: HashMap<usize, ObjectRef> = HashMap::new();
    let mut right_container: HashMap<usize, ObjectRef> = HashMap::new();
    for m in comp.matches() {
        if let Some(l) = m.left() {
            for child in crate::support::contents(l) {
                left_container.insert(key(&child), l.clone());
            }
        }
        if let Some(r) = m.right() {
            for child in crate::support::contents(r) {
                right_container.insert(key(&child), r.clone());
            }
        }
    }

    // Collect (dependent, requirement) edges first, then mutate the comparison
    // (keeps the immutable iteration and the mutable writes separated).
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for (i, d) in comp.differences().iter().enumerate() {
        match d.kind() {
            DiffKind::Add => {
                // Rule 1: ADD child depends on ADD parent.
                let Some(added) = d.right() else { continue };
                if let Some(container) = right_container.get(&key(added)) {
                    if let Some(&parent_diff) = add_diff_by_obj.get(&key(container)) {
                        edges.push((i, parent_diff));
                    }
                }
            }
            DiffKind::Delete => {
                // Rule 3: DELETE parent depends on DELETE children.
                let Some(removed) = d.left() else { continue };
                for child in crate::support::contents(removed) {
                    if let Some(&child_diff) = del_diff_by_obj.get(&key(&child)) {
                        edges.push((i, child_diff));
                    }
                }
            }
            DiffKind::Change => {
                // Rule 2: REFERENCE_CHANGE depends on target ADD.
                if d.type_() != DiffType::ReferenceChange {
                    continue;
                }
                if let Some(ref_obj) = d.new_value().and_then(single_object) {
                    if let Some(&target_diff) = add_diff_by_obj.get(&key(&ref_obj)) {
                        edges.push((i, target_diff));
                    }
                }
            }
            DiffKind::Move => {
                // Rule 4: MOVE depends on target ADD.
                if let Some(moved) = d.right() {
                    if let Some(&target_diff) = add_diff_by_obj.get(&key(moved)) {
                        edges.push((i, target_diff));
                    }
                }
            }
        }
    }
    for (source, target) in edges {
        add_requirement(comp, source, target, &mut seen);
    }
}

/// Add a deduplicated dependency edge.
fn add_requirement(
    comp: &mut Comparison,
    source: usize,
    target: usize,
    seen: &mut Vec<(usize, usize)>,
) {
    if source == target {
        return;
    }
    if seen.iter().any(|p| p.0 == source && p.1 == target) {
        return;
    }
    seen.push((source, target));
    comp.dependencies_mut()
        .push(Dependency::new(source, target));
}
