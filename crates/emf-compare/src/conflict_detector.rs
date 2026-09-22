//! `ConflictDetector` — 3-way conflict detection (aligned to Java
//! `org.eclipse.emf.compare.internal.ConflictDetector`, C++
//! `emf-compare/ConflictDetector`).
//!
//! For three-way comparisons, checks whether left and right both changed the
//! same feature relative to origin. If the resulting values are different the
//! conflict is `REAL`; if identical (a "pseudo" conflict) it is `PSEUDO`.

use crate::comparison::{Comparison, Conflict, ConflictKind};
use crate::diff::DiffType;
use crate::support::{key, object_list, value_equal};
use emf_common::value::{ObjectRef, Val};
use std::collections::{HashMap, HashSet};

/// Whether this is a three-way comparison and conflicts were detected.
pub(crate) fn detect_conflicts(comp: &mut Comparison) {
    if !comp.is_three_way() {
        return;
    }
    // Build side→origin and left→right maps from match origins.
    let mut left_to_origin: HashMap<usize, ObjectRef> = HashMap::new();
    let mut right_to_origin: HashMap<usize, ObjectRef> = HashMap::new();
    let mut left_to_right: HashMap<usize, ObjectRef> = HashMap::new();
    for m in comp.matches() {
        if let (Some(l), Some(o)) = (m.left(), m.origin()) {
            left_to_origin.insert(key(l), o.clone());
        }
        if let (Some(r), Some(o)) = (m.right(), m.origin()) {
            right_to_origin.insert(key(r), o.clone());
        }
        if let (Some(l), Some(r)) = (m.left(), m.right()) {
            left_to_right.insert(key(l), r.clone());
        }
    }

    let match_count = comp.matches().len();
    for i in 0..match_count {
        let (left, right, origin) = {
            let m = comp.match_at(i).expect("match");
            (m.left().cloned(), m.right().cloned(), m.origin().cloned())
        };
        let (Some(left), Some(right), Some(origin)) = (left, right, origin) else {
            continue;
        };
        let features = crate::support::all_features(&left);
        for sf in &features {
            if sf.is_derived() || sf.is_transient() || !sf.is_changeable() {
                continue;
            }
            let ov = e_get(&origin, sf.name());
            let lv = e_get(&left, sf.name());
            let rv = e_get(&right, sf.name());

            let (left_changed, right_changed, same_result) = if sf.is_reference() && sf.is_many() {
                let o_list = object_list(&ov);
                let l_list = object_list(&lv);
                let r_list = object_list(&rv);
                let (l_added, l_removed) = set_delta(&l_list, &o_list, &left_to_origin);
                let (r_added, r_removed) = set_delta(&r_list, &o_list, &right_to_origin);
                if l_added.is_empty()
                    && l_removed.is_empty()
                    && r_added.is_empty()
                    && r_removed.is_empty()
                {
                    continue;
                }
                let lc = !l_added.is_empty() || !l_removed.is_empty();
                let rc = !r_added.is_empty() || !r_removed.is_empty();
                if !lc || !rc {
                    continue;
                }
                // PSEUDO if left/right added/removed sets are equivalent.
                let added_equiv = added_equivalent(&l_added, &r_added, &left_to_right);
                let removed_equiv = removed_equivalent(&l_removed, &r_removed);
                (true, true, added_equiv && removed_equiv)
            } else {
                let lc = !value_equal(&lv, &ov);
                let rc = !value_equal(&rv, &ov);
                if !lc || !rc {
                    continue;
                }
                (true, true, value_equal(&lv, &rv))
            };
            let _ = (left_changed, right_changed);

            // Both changed.
            let mut c = Conflict::new(if same_result {
                ConflictKind::Pseudo
            } else {
                ConflictKind::Real
            });
            // Attach diffs targeting this feature (collect indices first to keep
            // the immutable reads and the mutable write separated).
            if let Some(m) = comp.match_at(i) {
                for &di in m.diff_indices() {
                    if let Some(d) = comp.diff_at(di) {
                        if d.attribute_name() == sf.name()
                            && (d.type_() == DiffType::AttributeChange
                                || d.type_() == DiffType::ReferenceChange)
                        {
                            c.push_diff(di);
                        }
                    }
                }
            }
            comp.conflicts_mut().push(c);
        }
    }
}

/// Set difference of a side list vs origin list (added / removed), resolving
/// identity via the side→origin map.
fn set_delta(
    side_list: &[ObjectRef],
    origin_list: &[ObjectRef],
    side_to_origin: &HashMap<usize, ObjectRef>,
) -> (Vec<ObjectRef>, Vec<ObjectRef>) {
    let contains_in_origin = |sobj: &ObjectRef| -> bool {
        for o in origin_list {
            if crate::support::is_same(o, sobj) {
                return true;
            }
            if let Some(map) = side_to_origin.get(&key(sobj)) {
                if crate::support::is_same(map, o) {
                    return true;
                }
            }
        }
        false
    };
    let contains_in_side = |oobj: &ObjectRef| -> bool {
        for s in side_list {
            if crate::support::is_same(s, oobj) {
                return true;
            }
            if let Some(map) = side_to_origin.get(&key(s)) {
                if crate::support::is_same(map, oobj) {
                    return true;
                }
            }
        }
        false
    };
    let mut added = Vec::new();
    let mut removed = Vec::new();
    for s in side_list {
        if !contains_in_origin(s) {
            added.push(s.clone());
        }
    }
    for o in origin_list {
        if !contains_in_side(o) {
            removed.push(o.clone());
        }
    }
    (added, removed)
}

fn added_equivalent(
    l_added: &[ObjectRef],
    r_added: &[ObjectRef],
    left_to_right: &HashMap<usize, ObjectRef>,
) -> bool {
    if l_added.len() != r_added.len() {
        return false;
    }
    let r_set: HashSet<usize> = r_added.iter().map(|o| key(o)).collect();
    for la in l_added {
        match left_to_right.get(&key(la)) {
            Some(mapped) => {
                if !r_set.contains(&key(mapped)) {
                    return false;
                }
            }
            None => return false,
        }
    }
    true
}

fn removed_equivalent(l_removed: &[ObjectRef], r_removed: &[ObjectRef]) -> bool {
    if l_removed.len() != r_removed.len() {
        return false;
    }
    let l_set: HashSet<usize> = l_removed.iter().map(|o| key(o)).collect();
    r_removed.iter().all(|rr| l_set.contains(&key(rr)))
}

/// Reflective read of a feature by name.
fn e_get(o: &ObjectRef, name: &str) -> Val {
    o.borrow().e_get(name).unwrap_or(Val::Null)
}
