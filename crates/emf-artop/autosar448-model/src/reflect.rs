//! Data-driven reflection over the generated AUTOSAR registry.
//!
//! Mirrors EMF semantics: `eSuperTypes` graph traversal, `eAllFeatures`,
//! `isSuperTypeOf`, `eGet`, plus the AUTOSAR-specific *serialization* lookups
//! (`xml.name` ↔ feature). Inheritance and reflection are metadata, not Rust
//! subtyping, so the 2105 classes of gautosar + autosar448 compile as plain
//! data tables in seconds.
//!
//! Everything the arxml loader/saver needs about a feature — its `xml.name`,
//! multiplicity, containment flag, APRXML role/type element flags and
//! `internal-xml-sequenceOffset` — lives in [`registry::FEATURE_META`].

use crate::registry::{
    ClassMeta, DataTypeMeta, EnumMeta, FeatureKind, FeatureMeta, TypeRef, ECLASS, FEATURE_META,
};

/// Minimal reflective value holder (representative subset; the serialization
/// layer maps real AUTOSAR attribute types onto these).
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

/// Global feature id (index into `registry::FEATURE_META`).
pub type FeatureId = u32;

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

    /// Set a feature value by ecore feature name. The feature may be declared on
    /// this class *or* inherited from any ancestor. Returns `None` if the name
    /// is not a feature anywhere in this class's hierarchy.
    pub fn set_by_name(&mut self, feature: &str, value: Val) -> Option<()> {
        let fid = find_feature(self.class_id, feature)?;
        self.set_by_id(fid, value);
        Some(())
    }

    /// Set a feature value by resolved feature id.
    pub fn set_by_id(&mut self, fid: FeatureId, value: Val) {
        if let Some(slot) = self.own.iter_mut().find(|(f, _)| *f == fid) {
            slot.1 = value;
        } else {
            self.own.push((fid, value));
        }
    }

    /// Read a feature value by resolved feature id (own storage only).
    pub fn get_by_id(&self, fid: FeatureId) -> Option<&Val> {
        self.own.iter().find(|(f, _)| *f == fid).map(|(_, v)| v)
    }
}

/// Class metadata by id.
pub fn class(id: u32) -> &'static ClassMeta {
    &ECLASS[id as usize]
}

/// Feature metadata by id.
pub fn feature(id: FeatureId) -> &'static FeatureMeta {
    &FEATURE_META[id as usize]
}

/// Look up a class id by (simple) ecore name.
pub fn class_id(name: &str) -> Option<u32> {
    crate::registry::name_to_id(name)
}

/// Look up a class id by its arxml element name (`xml.name`).
pub fn class_id_by_xml_name(xml: &str) -> Option<u32> {
    crate::registry::xml_name_to_id(xml)
}

/// Resolve an enum id by name.
pub fn enum_id(name: &str) -> Option<u32> {
    crate::registry::enum_id(name)
}

/// Enum metadata by id.
pub fn enum_meta(id: u32) -> EnumMeta {
    crate::registry::enum_meta(id)
}

/// Data type metadata by id.
pub fn datatype_meta(id: u32) -> DataTypeMeta {
    crate::registry::datatype_meta(id)
}

/// All transitive ancestor class ids (deduplicated, order unspecified).
/// Excludes the class itself.
pub fn all_super_ids(id: u32, out: &mut Vec<u32>) {
    for s in class(id).sups {
        if !out.contains(s) {
            out.push(*s);
            all_super_ids(*s, out);
        }
    }
}

/// EMF `eAllFeatures()`: ancestor-first feature id list (deduplicated). Own
/// features come last.
pub fn all_feature_ids(id: u32) -> Vec<FeatureId> {
    let mut order = Vec::new();
    all_super_ids(id, &mut order);
    order.push(id);
    let mut out = Vec::new();
    for c in order {
        for f in class(c).own {
            if !out.contains(f) {
                out.push(*f);
            }
        }
    }
    out
}

/// Resolve a feature (by ecore name) in the class's `eAllFeatures` scope.
pub fn find_feature(id: u32, name: &str) -> Option<FeatureId> {
    all_feature_ids(id)
        .into_iter()
        .find(|&f| feature(f).name == name)
}

/// Resolve a feature by arxml element name: tries `xml.name`, then
/// `xml.namePlural`, then the ecore feature name (mirrors the C++ handler's
/// "feature name first, xml.name fallback" rule).
pub fn find_feature_by_xml(id: u32, xml_name: &str) -> Option<FeatureId> {
    let scope = all_feature_ids(id);
    scope
        .iter()
        .copied()
        .find(|&f| feature(f).xml_name == xml_name)
        .or_else(|| {
            scope
                .iter()
                .copied()
                .find(|&f| feature(f).xml_name_plural == xml_name)
        })
        .or_else(|| scope.iter().copied().find(|&f| feature(f).name == xml_name))
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

/// EMF `eGet`: resolve a feature by id across the hierarchy and read it from the
/// object (own storage only).
pub fn e_get(obj: &RObject, fid: FeatureId) -> Option<Val> {
    obj.get_by_id(fid).cloned()
}

/// Whether the feature is many-valued (`upperBound != 1`).
pub fn is_many(fid: FeatureId) -> bool {
    feature(fid).upper != 1
}

/// Whether the feature is a containment `EReference`.
pub fn is_containment(fid: FeatureId) -> bool {
    let f = feature(fid);
    f.kind == FeatureKind::Reference && f.containment
}

/// Whether the feature is a non-containment `EReference` (a cross reference).
pub fn is_cross_reference(fid: FeatureId) -> bool {
    let f = feature(fid);
    f.kind == FeatureKind::Reference && !f.containment
}

/// Resolved target class of a reference feature, if any.
pub fn reference_target(fid: FeatureId) -> Option<u32> {
    match feature(fid).ty {
        TypeRef::Class(c) => Some(c),
        _ => None,
    }
}

/// Number of self-inheritance cycles over the whole graph (0 for AUTOSAR).
pub fn count_cycles() -> usize {
    (0..ECLASS.len())
        .filter(|&i| {
            let mut v = Vec::new();
            all_super_ids(i as u32, &mut v);
            v.contains(&(i as u32))
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inheritance_graph_is_cycle_free() {
        assert_eq!(count_cycles(), 0);
    }

    #[test]
    fn generic_base_is_reachable_from_autosar_class() {
        // ARObject (autosar448) inherits GARObject (gautosar) — the cross-file
        // link the merged registry must preserve.
        let arobject = class_id("ARObject").unwrap();
        let garobject = class_id("GARObject").unwrap();
        assert!(is_super_type_of(garobject, arobject));
        assert!(!is_super_type_of(arobject, garobject));
    }

    #[test]
    fn swc_implementation_reaches_both_files() {
        let swc = class_id("SwcImplementation").unwrap();
        assert!(is_super_type_of(class_id("Referrable").unwrap(), swc));
        assert!(is_super_type_of(class_id("GReferrable").unwrap(), swc));
        let mut anc = Vec::new();
        all_super_ids(swc, &mut anc);
        // the full ancestor set spans gautosar + autosar448
        assert!(anc.len() >= 8, "ancestors = {}", anc.len());
    }

    #[test]
    fn xml_name_lookup_matches_arxml_element() {
        // Referrable's shortName feature serializes as <SHORT-NAME>.
        let referrable = class_id("Referrable").unwrap();
        let fid = find_feature_by_xml(referrable, "SHORT-NAME").unwrap();
        assert_eq!(feature(fid).name, "shortName");
        assert_eq!(feature(fid).xml_name, "SHORT-NAME");
        // AR-PACKAGE (multi containment) resolves on the AUTOSAR root class.
        let autosar = class_id("AUTOSAR").unwrap();
        let arpkg = find_feature_by_xml(autosar, "AR-PACKAGE").unwrap();
        assert_eq!(feature(arpkg).name, "arPackages");
        assert!(is_containment(arpkg));
        assert!(is_many(arpkg));
        assert_eq!(
            reference_target(arpkg),
            Some(class_id("ARPackage").unwrap())
        );
    }

    #[test]
    fn eget_reads_ancestor_feature() {
        let swc = class_id("SwcImplementation").unwrap();
        let mut obj = RObject::new(swc);
        assert!(obj
            .set_by_name("shortName", Val::S("MySwc".into()))
            .is_some());
        let fid = find_feature(swc, "shortName").unwrap();
        assert!(matches!(e_get(&obj, fid), Some(Val::S(s)) if s == "MySwc"));
    }

    #[test]
    fn reject_features_out_of_scope() {
        let swc = class_id("SwcImplementation").unwrap();
        let mut obj = RObject::new(swc);
        assert!(obj.set_by_name("__no_such_feature__", Val::Null).is_none());
    }
}
