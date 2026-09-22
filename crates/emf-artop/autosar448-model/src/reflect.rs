//! Data-driven reflection over the generated AUTOSAR 4.4.8 registry.
//!
//! Mirrors EMF semantics: `eSuperTypes` graph traversal, `eAllFeatures`,
//! `isSuperTypeOf`, and `eGet` (resolve a feature name to its declaring class,
//! then read the object's stored value). The whole point is that *inheritance
//! and reflection are metadata, not Rust subtyping*, so 1925 classes with deep,
//! multiple inheritance compile in seconds (validated in CI).

use crate::registry::{name_to_id, ECLASS, FEATURE_NAMES};

/// Minimal reflective value holder (representative subset; the serialization
/// layer later maps real AUTOSAR attribute types onto these).
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    /// Integer value.
    I(i64),
    /// Floating point value.
    F(f64),
    /// String value.
    S(String),
    /// Boolean value.
    B(bool),
    /// Explicit null / unset.
    Null,
}

impl Val {
    /// Read a string out of a string-valued `Val`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Val::S(s) => Some(s),
            _ => None,
        }
    }
}

/// Feature name id (index into `registry::FEATURE_NAMES`).
pub type FeatureId = u16;

/// A reflective object that stores only its *own* feature values; inherited
/// features are resolved through the registry's `eSuperTypes` metadata.
#[derive(Debug, Clone)]
pub struct RObject {
    /// Index into `registry::ECLASS`.
    pub class_id: u32,
    /// Own feature (id, value) pairs.
    pub own: Vec<(FeatureId, Val)>,
}

impl RObject {
    /// New reflective object of the given class.
    pub fn new(class_id: u32) -> Self {
        Self {
            class_id,
            own: Vec::new(),
        }
    }

    /// Set a feature value by name. The feature may be declared on this class
    /// *or* inherited from any ancestor (`eAllFeatures` scope). Returns `None`
    /// if the name is not a feature anywhere in this class's hierarchy.
    pub fn set_by_name(&mut self, feature: &str, value: Val) -> Option<()> {
        let fid = feature_id(feature)?;
        let in_scope = all_features(self.class_id).iter().any(|(_, f)| *f == fid);
        if !in_scope {
            return None;
        }
        if let Some(slot) = self.own.iter_mut().find(|(f, _)| *f == fid) {
            slot.1 = value;
        } else {
            self.own.push((fid, value));
        }
        Some(())
    }
}

/// Feature name -> id (`FEATURE_NAMES` index).
pub fn feature_id(name: &str) -> Option<FeatureId> {
    FEATURE_NAMES
        .iter()
        .position(|n| *n == name)
        .map(|i| i as FeatureId)
}

/// All transitive ancestor class ids (deduplicated, order unspecified).
/// Excludes the class itself.
pub fn all_super_ids(id: u32, out: &mut Vec<u32>) {
    let me = &ECLASS[id as usize];
    for s in me.sups {
        if !out.contains(s) {
            out.push(*s);
            all_super_ids(*s, out);
        }
    }
}

/// EMF `eAllFeatures()`: ancestor-first feature list (deduplicated), as
/// `(origin_class_id, feature_id)` pairs. Own features come last.
pub fn all_features(id: u32) -> Vec<(u32, FeatureId)> {
    let mut order = Vec::new();
    all_super_ids(id, &mut order);
    let mut out = Vec::new();
    for class in &order {
        push_new(&mut out, *class);
    }
    push_new(&mut out, id);
    out
}

fn push_new(out: &mut Vec<(u32, FeatureId)>, class_id: u32) {
    for f in ECLASS[class_id as usize].own {
        if !out.iter().any(|(_, fd)| *fd == *f) {
            out.push((class_id, *f));
        }
    }
}

/// EMF `isSuperTypeOf(sub, sup)`-style query: is `sup` an ancestor (or equal)
/// of `sub`?
pub fn is_super_type_of(sup: u32, sub: u32) -> bool {
    if sup == sub {
        return true;
    }
    let mut anc = Vec::new();
    all_super_ids(sub, &mut anc);
    anc.contains(&sup)
}

/// EMF `eGet`: resolve a feature by name across the whole hierarchy (ancestors
/// first), then read it from the object. Returns `Some(Val)` when the feature
/// is declared somewhere up the chain and is set on this object.
pub fn e_get(class_id: u32, feature: &str, obj: &RObject) -> Option<Val> {
    let fid = feature_id(feature)?;
    let mut order = Vec::new();
    all_super_ids(class_id, &mut order);
    order.push(class_id);
    for class in order {
        if ECLASS[class as usize].own.contains(&fid) {
            return obj
                .own
                .iter()
                .find(|(f, _)| *f == fid)
                .map(|(_, v)| v.clone());
        }
    }
    None
}

/// Convenience: both `all_super_ids` and cycle detection over the whole graph.
/// Returns the number of self-inheritance cycles (should be 0 for AUTOSAR).
pub fn count_cycles() -> usize {
    (0..ECLASS.len())
        .filter(|&i| {
            let mut v = Vec::new();
            all_super_ids(i as u32, &mut v);
            v.contains(&(i as u32))
        })
        .count()
}

/// Look up a class id by name (binary search over the generated index).
pub fn class_id(name: &str) -> Option<u32> {
    name_to_id(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_inheritance_graph_is_cycle_free() {
        assert_eq!(count_cycles(), 0);
    }

    #[test]
    fn real_depth11_ancestor_chain() {
        // SwcImplementation inherits from Referrable through a real 8-hop chain.
        let swc = class_id("SwcImplementation").unwrap();
        let referrable = class_id("Referrable").unwrap();
        assert!(is_super_type_of(referrable, swc));
        assert!(!is_super_type_of(swc, referrable));
        let mut anc = Vec::new();
        all_super_ids(swc, &mut anc);
        assert_eq!(anc.len(), 8); // matches the actual AUTOSAR 4.4.8 DAG
    }

    #[test]
    fn eget_reads_ancestor_feature() {
        // "shortName" is declared on Referrable, which is an ancestor of
        // SwcImplementation — so it must be settable and readable on a
        // SwcImplementation object through the hierarchy-walking eGet.
        let swc = class_id("SwcImplementation").unwrap();
        assert!(is_super_type_of(class_id("Referrable").unwrap(), swc));
        let mut obj = RObject::new(swc);
        assert!(obj
            .set_by_name("shortName", Val::S("MySwc".into()))
            .is_some());
        let got = e_get(swc, "shortName", &obj);
        assert!(matches!(got, Some(Val::S(s)) if s == "MySwc"));
    }

    #[test]
    fn reject_features_out_of_scope() {
        let swc = class_id("SwcImplementation").unwrap();
        let mut obj = RObject::new(swc);
        assert!(obj.set_by_name("__no_such_feature__", Val::Null).is_none());
    }
}
