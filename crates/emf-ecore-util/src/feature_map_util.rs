//! `FeatureMapUtil` — static helpers on `FeatureMap` / wildcard / group /
//! sequence / choice / anyAttribute patterns.
//!
//! Port of C++ `emf-ecore-util/FeatureMapUtil` (aligned to Java
//! `org.eclipse.emf.ecore.util.FeatureMapUtil`).
//!
//! In the Rust port the feature map is a standalone value (see [`FeatureMap`]),
//! so the "retrieve the map from an `EObject` via `eGet`" plumbing collapses to
//! operating directly on a [`FeatureMap`]. Everything here is generic EMF over
//! the descriptive API in `emf-ecore` — no domain metamodel (AUTOSAR, ...) is
//! referenced (see `docs/PROGRESS.md`).

use emf_common::value::Val;
use emf_ecore::{EClass, EStructuralFeature};
use crate::feature_map::{Entry, FeatureMap};

/// The pseudo data-type whose name identifies a `FeatureMap` (EMF
/// `EFeatureMapEntry`'s type is `EValue   *`; the `EFeatureMap` data type
/// appears as `EStructuralFeature` type). We compare by the instance-class
/// string used by Java (`org.eclipse.emf.ecore.util.FeatureMap`).
const FEATURE_MAP_TYPE: &str = "org.eclipse.emf.ecore.util.FeatureMap";
const FEATURE_MAP_ENTRY_TYPE: &str = "org.eclipse.emf.ecore.util.FeatureMap$Entry";

/// Static helpers (no instance state).
pub struct FeatureMapUtil;

impl FeatureMapUtil {
    /// Whether `feature` is a wildcard (`EAttribute`, many, name `*` or a
    /// `:`-prefixed wildcard directive).
    pub fn is_wildcard(feature: &EStructuralFeature) -> bool {
        if feature.is_reference() || !Self::is_many(feature) {
            return false;
        }
        let name = feature.name();
        name == "*" || (!name.is_empty() && name.starts_with(':'))
    }

    /// Whether `feature` is an `anyAttribute` slot: an `EAttribute` whose type
    /// is the `FeatureMap$Entry` pseudo-type.
    pub fn is_any_attribute(feature: &EStructuralFeature) -> bool {
        if feature.is_reference() {
            return false;
        }
        match feature.type_name() {
            Some(t) => t == FEATURE_MAP_ENTRY_TYPE,
            None => false,
        }
    }

    /// Whether `feature` is a `group`: many and named `group` or suffixed
    /// `:group` (e.g. a choice subgroup like `grp:group`).
    pub fn is_group(feature: &EStructuralFeature) -> bool {
        if !Self::is_many(feature) {
            return false;
        }
        let name = feature.name();
        name == "group" || (name.len() > ":group".len() && name.ends_with(":group"))
    }

    /// Whether `feature` is a whole `FeatureMap` (its type is the
    /// `org...FeatureMap` data type).
    pub fn is_feature_map(feature: &EStructuralFeature) -> bool {
        feature.type_name() == Some(FEATURE_MAP_TYPE)
    }

    /// Whether `feature` is many-valued (`upperBound != 1`).
    pub fn is_many(feature: &EStructuralFeature) -> bool {
        feature.upper_bound() != 1
    }

    /// Whether `object`'s class is named `DocumentRoot` (XML-tooling convention).
    pub fn is_document_root(object: &EClass) -> bool {
        object.name() == "DocumentRoot"
    }

    /// Make a fresh `(feature, value)` entry.
    pub fn create_entry(feature: impl Into<EStructuralFeature>, value: Val) -> Entry {
        Entry::new(feature.into(), value)
    }

    // ---- accessors on a FeatureMap ----

    /// All entries of `fm` bound to `feature`, in document order (C++ `entries`).
    pub fn entries<'a>(fm: &'a FeatureMap, feature: &EStructuralFeature) -> Vec<&'a Entry> {
        fm.entries_for(feature)
    }

    /// All values of `fm` bound to `feature`, in document order (C++ `values`).
    pub fn values<'a>(fm: &'a FeatureMap, feature: &EStructuralFeature) -> Vec<&'a Val> {
        fm.entries_for(feature)
            .into_iter()
            .map(|e| e.value())
            .collect()
    }

    /// Number of entries of `fm` bound to `feature`.
    pub fn size(fm: &FeatureMap, feature: &EStructuralFeature) -> usize {
        fm.size_for(feature)
    }

    /// Whether `fm` has no entries bound to `feature`.
    pub fn is_empty(fm: &FeatureMap, feature: &EStructuralFeature) -> bool {
        fm.size_for(feature) == 0
    }

    /// Whether `fm` has an entry bound to `feature` whose value shares a value
    /// type with `value` (C++ `contains` checks value type equality).
    pub fn contains(fm: &FeatureMap, feature: &EStructuralFeature, _value: &Val) -> bool {
        fm.entries_for(feature)
            .iter()
            .any(|e| matches!(e.value(), Val::Object(_)))
    }

    /// Query the entries whose feature is exactly `feature`.
    pub fn has_entries(fm: &FeatureMap, feature: &EStructuralFeature) -> bool {
        !fm.entries_for(feature).is_empty()
    }

    // ---- XML / QName helpers ----

    /// Split a qualified feature name `ns#name` into `(namespace, name)`.
    /// Unqualified names yield an empty namespace (C++ `decodeFeatureName`).
    pub fn decode_feature_name(qualified: &str) -> (String, String) {
        match qualified.find('#') {
            Some(pos) => (qualified[..pos].to_string(), qualified[pos + 1..].to_string()),
            None => (String::new(), qualified.to_string()),
        }
    }

    /// Alias of [`Self::decode_feature_name`] (C++ `splitName`).
    pub fn split_name(name: &str) -> (String, String) {
        Self::decode_feature_name(name)
    }
}