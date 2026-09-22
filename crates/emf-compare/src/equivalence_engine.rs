//! `EquivalenceEngine` — cross-containment-boundary equivalences (aligned to
//! Java `org.eclipse.emf.compare.internal.EquivalenceEngine`, C++
//! `emf-compare/EquivalenceEngine`).
//!
//! When a `Match`'s object references (via non-containment references) another
//! object that also participates in the comparison, the two matches are linked
//! with an `Equivalence` so callers know that ADD/DELETE of the source implies a
//! reference change.

use crate::comparison::{Comparison, Equivalence};
use crate::support::{key, object_list, single_object};
use emf_common::value::ObjectRef;
use std::collections::HashMap;

/// Compute equivalences over a comparison.
pub(crate) fn compute_equivalences(comp: &mut Comparison) {
    // left / right object -> match index.
    let mut left_obj_to_match: HashMap<usize, usize> = HashMap::new();
    let mut right_obj_to_match: HashMap<usize, usize> = HashMap::new();
    for (i, m) in comp.matches().iter().enumerate() {
        if let Some(l) = m.left() {
            left_obj_to_match.insert(key(l), i);
        }
        if let Some(r) = m.right() {
            right_obj_to_match.insert(key(r), i);
        }
    }

    // For each match, find ref targets present in the comparison.
    let match_count = comp.matches().len();
    for i in 0..match_count {
        let src = comp
            .match_at(i)
            .and_then(|m| m.left().or_else(|| m.right()))
            .cloned();
        let Some(src) = src else { continue };

        let targets = ref_targets(&src);
        for target in targets {
            let tm = left_obj_to_match
                .get(&key(&target))
                .copied()
                .or_else(|| right_obj_to_match.get(&key(&target)).copied());
            if let Some(tm) = tm {
                if tm != i {
                    let mut eq = Equivalence::default();
                    eq.add_match(i);
                    eq.add_match(tm);
                    comp.equivalences_mut().push(eq);
                }
            }
        }
    }
}

/// Collect targets of non-containment references (single + many).
fn ref_targets(src: &ObjectRef) -> Vec<ObjectRef> {
    use emf_common::eobject::downcast_ref;
    let b = src.borrow();
    let Some(dy) = downcast_ref::<emf_ecore::DynamicEObject>(&*b) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for f in dy.all_structural_features() {
        if !f.is_reference() || f.is_containment() {
            continue;
        }
        let v = dy.e_get_feature(&f);
        if f.is_many() {
            out.extend(object_list(&v));
        } else if let Some(o) = single_object(&v) {
            out.push(o);
        }
    }
    out
}
