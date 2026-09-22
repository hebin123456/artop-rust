//! `MatchEngine` — 2-way and 3-way object matching (aligned to Java
//! `org.eclipse.emf.compare.match.DefaultMatchEngine`, C++
//! `emf-compare/MatchEngine`).
//!
//! Ported over the reflection surface. Uses the same strategy hierarchy as the
//! C++/Java source:
//! - ID-driven matching (manual IDs > external provider > auto ID attribute)
//!   for objects that carry an explicit identifier — no proximity fallback.
//! - Proximity (greedy best-similarity) matching within same-`EClass` buckets
//!   for objects without an ID (aligned to `ProximityEObjectMatcher`),
//!   with an O(1) same-index fast path and a perfect-match early stop.
//! - 3-way: 2-way left-right match first, then origin-left structural-position
//!   matching to populate each match's origin.

use crate::comparison::{Comparison, IdentifierProvider, MatchKind};
use crate::support::{all_features, contents, is_same, key, value_equal};
use emf_common::value::{ObjectRef, Val};
use std::collections::HashMap;

/// Thread-safe defaults matching Java `DefaultMatchEngine`.
pub struct MatchEngine {
    threshold: f64,
    use_id_matcher: bool,
    use_id_attribute: bool,
    /// Manual id map: object key -> id.
    id_map: HashMap<usize, String>,
    /// External identifier provider.
    id_provider: Option<Box<IdentifierProvider>>,
}

impl Default for MatchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl MatchEngine {
    /// A new engine with default configuration.
    pub fn new() -> Self {
        Self {
            threshold: 1.0,
            use_id_matcher: false,
            use_id_attribute: true,
            id_map: HashMap::new(),
            id_provider: None,
        }
    }

    /// `setSimilarityThreshold`.
    pub fn set_similarity_threshold(&mut self, t: f64) {
        self.threshold = t;
    }
    /// `getSimilarityThreshold`.
    pub fn similarity_threshold(&self) -> f64 {
        self.threshold
    }
    /// `setUseIdentifierMatcher`.
    pub fn set_use_identifier_matcher(&mut self, b: bool) {
        self.use_id_matcher = b;
    }
    /// `getUseIdentifierMatcher`.
    pub fn use_identifier_matcher(&self) -> bool {
        self.use_id_matcher
    }
    /// `setUseIdAttribute`.
    pub fn set_use_id_attribute(&mut self, b: bool) {
        self.use_id_attribute = b;
    }
    /// `getUseIdAttribute`.
    pub fn use_id_attribute(&self) -> bool {
        self.use_id_attribute
    }
    /// `registerIdentifier`.
    pub fn register_identifier(&mut self, obj: &ObjectRef, id: impl Into<String>) {
        self.id_map.insert(key(obj), id.into());
    }
    /// `getIdentifier`.
    pub fn identifier(&self, obj: &ObjectRef) -> Option<String> {
        self.id_map.get(&key(obj)).map(|s| s.clone())
    }
    /// `clearIdentifiers`.
    pub fn clear_identifiers(&mut self) {
        self.id_map.clear();
    }
    /// `setIdentifierProvider`.
    pub fn set_identifier_provider(&mut self, p: Box<IdentifierProvider>) {
        self.id_provider = Some(p);
    }

    /// 2-way match over two containment trees.
    pub fn match_2way(
        &mut self,
        left: Option<&ObjectRef>,
        right: Option<&ObjectRef>,
        comp: &mut Comparison,
    ) {
        match_pair(self, left, right, comp);
    }

    /// 3-way match over left/right/origin trees.
    pub fn match_3way(
        &mut self,
        left: Option<&ObjectRef>,
        right: Option<&ObjectRef>,
        origin: Option<&ObjectRef>,
        comp: &mut Comparison,
    ) {
        comp.set_three_way(true);
        match_pair(self, left, right, comp);
        // Origin-left structural-position matching.
        if let Some(origin) = origin {
            if let Some(left) = left {
                let mut left_to_origin: HashMap<usize, ObjectRef> = HashMap::new();
                match_origin_recursive(self, left, origin, &mut left_to_origin);
                for i in 0..comp.matches().len() {
                    let m_left = comp.match_at(i).and_then(|m| m.left());
                    if let Some(ml) = m_left {
                        if let Some(o) = left_to_origin.get(&key(ml)) {
                            if let Some(m) = comp.matches_mut().get_mut(i) {
                                m.set_origin(Some(o.clone()));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Match a single object pair, returning its match index.
    pub fn match_one(
        &mut self,
        left: Option<&ObjectRef>,
        right: Option<&ObjectRef>,
        comp: &mut Comparison,
    ) -> Option<usize> {
        match_one_inner(self, left, right, comp)
    }

    /// Resolve the ID of an object, if it has one.
    pub fn resolve_id(&self, obj: &ObjectRef) -> (String, bool) {
        // 1) manual registerIdentifier
        if let Some(id) = self.id_map.get(&key(obj)) {
            let id = id.clone();
            let present = !id.is_empty();
            return (id, present);
        }
        // 2) external provider
        if let Some(p) = &self.id_provider {
            let id = p(obj);
            if !id.is_empty() {
                return (id, true);
            }
        }
        // 3) auto ID attribute
        if self.use_id_attribute {
            let auto = auto_id(obj);
            if !auto.is_empty() {
                return (auto, true);
            }
        }
        (String::new(), false)
    }
}

/// Match a pair of roots (the top-level entry of both 2-way and 3-way).
fn match_pair(
    me: &mut MatchEngine,
    left: Option<&ObjectRef>,
    right: Option<&ObjectRef>,
    comp: &mut Comparison,
) {
    let root = match_one_inner(me, left, right, comp);
    let left = match left {
        Some(l) => l,
        None => return,
    };
    let right = match right {
        Some(r) => r,
        None => return,
    };
    if root.is_none() {
        return;
    }

    let left_contents = contents(left);
    let right_contents = contents(right);
    let n_right = right_contents.len();
    let mut right_used: Vec<bool> = vec![false; n_right];
    let mut right_has_id: Vec<bool> = vec![false; n_right];

    let id_match_enabled = me.use_id_matcher || me.use_id_attribute || me.id_provider.is_some();

    // ID index for the right side: (class name, id) -> position.
    let mut right_id_index: HashMap<String, usize> = HashMap::new();
    if id_match_enabled {
        for j in 0..n_right {
            let (id, has_id) = me.resolve_id(&right_contents[j]);
            if has_id {
                right_has_id[j] = true;
                let cls = class_name(&right_contents[j]);
                right_id_index.insert(cls + "\u{1}" + &id, j);
            }
        }
    }

    // Type buckets for proximity: class name -> right positions without id.
    let mut right_type_buckets: HashMap<String, Vec<usize>> = HashMap::new();
    for j in 0..n_right {
        if right_has_id[j] || right_used[j] {
            continue;
        }
        let cls = class_name(&right_contents[j]);
        right_type_buckets.entry(cls).or_default().push(j);
    }

    // 1) match each left child.
    for li in 0..left_contents.len() {
        let lc = &left_contents[li];
        let (left_id, has_left_id) = if id_match_enabled {
            me.resolve_id(lc)
        } else {
            (String::new(), false)
        };

        let mut matched = false;
        if has_left_id {
            let cls = class_name(lc);
            let key_str = cls + "\u{1}" + &left_id;
            if let Some(&idx) = right_id_index.get(&key_str) {
                if !right_used[idx] {
                    right_used[idx] = true;
                    match_pair(me, Some(lc), Some(&right_contents[idx]), comp);
                    matched = true;
                }
            }
        } else {
            // Proximity matching within same-type bucket, with same-index fast path.
            let mut best_idx: Option<usize> = None;
            let mut best_sim = -1.0;
            let lcc = class_name(lc);

            // Same-position first: O(1) fast path for identical layouts.
            if li < n_right && !right_used[li] && !right_has_id[li] {
                let rc = &right_contents[li];
                if class_name(rc) == lcc {
                    let sim = compute_similarity(lc, rc);
                    best_idx = Some(li);
                    best_sim = sim;
                }
            }
            if best_sim < 1.0 {
                if let Some(bucket) = right_type_buckets.get(&lcc) {
                    for &j in bucket {
                        if right_used[j] || Some(j) == best_idx {
                            continue;
                        }
                        let sim = compute_similarity(lc, &right_contents[j]);
                        if sim > best_sim {
                            best_sim = sim;
                            best_idx = Some(j);
                        }
                        if best_sim >= 1.0 {
                            break;
                        }
                    }
                }
            }
            if let Some(idx) = best_idx {
                // Re-check: the index must still be unused (update may have moved).
                if !right_used[idx] {
                    right_used[idx] = true;
                    match_pair(me, Some(lc), Some(&right_contents[idx]), comp);
                    matched = true;
                }
            }
        }

        if !matched {
            match_pair(me, Some(lc), None, comp);
        }
    }
    // 2) unmatched right children -> ABSENT_LEFT (ADD).
    for j in 0..n_right {
        if !right_used[j] {
            match_pair(me, None, Some(&right_contents[j]), comp);
        }
    }
}

/// Match a single object pair, producing one match entry.
fn match_one_inner(
    me: &mut MatchEngine,
    left: Option<&ObjectRef>,
    right: Option<&ObjectRef>,
    comp: &mut Comparison,
) -> Option<usize> {
    match (left, right) {
        (Some(l), Some(r)) if is_same(l, r) => {
            return Some(comp.add_match(
                Some(l.clone()),
                Some(r.clone()),
                MatchKind::Identical,
                1.0,
            ));
        }
        (None, Some(r)) => {
            return Some(comp.add_match(None, Some(r.clone()), MatchKind::AbsentLeft, 0.0));
        }
        (Some(l), None) => {
            return Some(comp.add_match(Some(l.clone()), None, MatchKind::AbsentRight, 0.0));
        }
        (None, None) => return None,
        _ => {}
    }

    let (l, r) = (left.unwrap(), right.unwrap());

    // Identifier matcher (xmi:id) — cross-file.
    if me.use_id_matcher {
        let lid = me.id_map.get(&key(l)).cloned();
        let rid = me.id_map.get(&key(r)).cloned();
        if let (Some(lid), Some(rid)) = (lid, rid) {
            if !lid.is_empty() && lid == rid && class_name(l) == class_name(r) {
                return Some(comp.add_match(
                    Some(l.clone()),
                    Some(r.clone()),
                    MatchKind::Identical,
                    1.0,
                ));
            }
        }
    }

    // Compare EClass names.
    let lc = class_name(l);
    let rc = class_name(r);
    if lc != rc {
        return Some(comp.add_match(Some(l.clone()), Some(r.clone()), MatchKind::Different, 0.0));
    }

    let sim = compute_similarity(l, r);
    let kind = if sim >= me.threshold {
        MatchKind::Identical
    } else {
        MatchKind::Different
    };
    Some(comp.add_match(Some(l.clone()), Some(r.clone()), kind, sim))
}

/// Recursively match origin children to left children by structural position.
fn match_origin_recursive(
    me: &mut MatchEngine,
    left: &ObjectRef,
    origin: &ObjectRef,
    left_to_origin: &mut HashMap<usize, ObjectRef>,
) {
    if class_name(left) != class_name(origin) {
        return;
    }
    left_to_origin.insert(key(left), origin.clone());

    let left_children = contents(left);
    let origin_children = contents(origin);
    let n_origin = origin_children.len();
    let mut origin_used: Vec<bool> = vec![false; n_origin];
    let mut origin_buckets: HashMap<String, Vec<usize>> = HashMap::new();
    for j in 0..n_origin {
        let cls = class_name(&origin_children[j]);
        origin_buckets.entry(cls).or_default().push(j);
    }
    for lchild in &left_children {
        let mut best_idx: Option<usize> = None;
        let mut best_sim = -1.0;
        let lcc = class_name(lchild);
        if let Some(bucket) = origin_buckets.get(&lcc) {
            for &j in bucket {
                if origin_used[j] {
                    continue;
                }
                let sim = compute_similarity(lchild, &origin_children[j]);
                if sim > best_sim {
                    best_sim = sim;
                    best_idx = Some(j);
                }
                if best_sim >= 1.0 {
                    break;
                }
            }
        }
        if let Some(idx) = best_idx {
            origin_used[idx] = true;
            match_origin_recursive(me, lchild, &origin_children[idx], left_to_origin);
        }
    }
}

// ---- similarity / id helpers (aligned to C++ anonymous namespace) ----

/// The class name of an object.
fn class_name(o: &ObjectRef) -> String {
    o.borrow().e_class().to_string()
}

/// Auto ID from the class's ID attribute (aligned to `EcoreUtil.getID`).
fn auto_id(obj: &ObjectRef) -> String {
    let b = obj.borrow();
    use emf_common::eobject::downcast_ref;
    let Some(dy) = downcast_ref::<emf_ecore::DynamicEObject>(&*b) else {
        return String::new();
    };
    let id_feature = match dy.class().id_feature() {
        Some(id) => id,
        None => return String::new(),
    };
    for f in dy.all_structural_features() {
        if f.is_reference() || f.feature_id() != id_feature {
            continue;
        }
        let v = dy.e_get_feature(&f);
        return scalar_to_id(&v);
    }
    String::new()
}

/// Best-effort scalar-to-string for ID values.
fn scalar_to_id(v: &Val) -> String {
    match v {
        Val::String(s) => s.clone(),
        Val::Int(i) => i.to_string(),
        Val::EnumLiteral(e) => e.clone(),
        _ => String::new(),
    }
}

/// Compute similarity (0..1) between two objects over single-valued
/// comparable features (aligned to `AbstractSimilarityChecker`).
fn compute_similarity(left: &ObjectRef, right: &ObjectRef) -> f64 {
    if is_same(left, right) {
        return 1.0;
    }
    let lc = class_name(left);
    let rc = class_name(right);
    if lc != rc {
        return 0.0;
    }

    let l_features = all_features(left);
    let mut similar = 0.0;
    let mut total = 0.0;
    for sf in &l_features {
        if !sf.is_changeable() || sf.is_derived() || sf.is_transient() {
            continue;
        }
        if sf.is_reference() {
            // Only single-valued, non-containment references participate.
            if sf.is_many() || sf.is_containment() {
                continue;
            }
            let lv = obj_e_get(left, sf.name());
            let rv = obj_e_get(right, sf.name());
            // Skip non-proxy cross-object references (no match map yet).
            if let (Some(lo), Some(ro)) = (single_object(&lv), single_object(&rv)) {
                if !lo.borrow().e_is_proxy() && !ro.borrow().e_is_proxy() && !is_same(&lo, &ro) {
                    continue;
                }
            }
        }
        total += 1.0;
        if value_equal(&obj_e_get(left, sf.name()), &obj_e_get(right, sf.name())) {
            similar += 1.0;
        }
    }
    if total > 0.0 {
        similar / total
    } else {
        1.0
    }
}

/// Reflective read of a feature by name.
fn obj_e_get(o: &ObjectRef, name: &str) -> Val {
    o.borrow().e_get(name).unwrap_or(Val::Null)
}

use crate::support::single_object;

/// The public top-level 2-way/3-way match entry used by `Comparison::compare`.
pub(crate) fn do_match(
    left: Option<&ObjectRef>,
    right: Option<&ObjectRef>,
    origin: Option<&ObjectRef>,
    comp: &mut Comparison,
) {
    let mut me = MatchEngine::new();
    if origin.is_some() {
        me.match_3way(left, right, origin, comp);
    } else {
        me.match_2way(left, right, comp);
    }
}
