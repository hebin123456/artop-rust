//! EMF `FeatureMap` / `FeatureMapEntry`, ported from C++ `emf-common/FeatureMap`.
//!
//! A `FeatureMap` backs volatile + transient + derived features (extended
//! features, wrapper elements, mixed containers). Each [`FeatureMapEntry`]
//! associates an `EStructuralFeature` (by name + a map feature id) with a value.

use crate::value::Val;

/// A single mixed-container entry.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureMapEntry {
    /// The structural feature name this entry belongs to.
    pub feature: String,
    /// Value (object reference or atomic).
    pub value: Val,
}

impl FeatureMapEntry {
    /// New entry.
    pub fn new(feature: impl Into<String>, value: Val) -> Self {
        Self {
            feature: feature.into(),
            value,
        }
    }
}

/// EMF `FeatureMap`: a filtered view over a list of entries.
#[derive(Debug, Clone, Default)]
pub struct FeatureMap {
    entries: Vec<FeatureMapEntry>,
}

impl FeatureMap {
    /// New empty map.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Const empty (for default impl in `EObject::feature_map`).
    pub const fn new_const() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Append an entry.
    pub fn add(&mut self, entry: FeatureMapEntry) {
        self.entries.push(entry);
    }

    /// Remove all entries for `feature`.
    pub fn remove_by_feature(&mut self, feature: &str) {
        self.entries.retain(|e| e.feature != feature);
    }

    /// All entries for `feature`.
    pub fn list(&self, feature: &str) -> Vec<FeatureMapEntry> {
        self.entries
            .iter()
            .filter(|e| e.feature == feature)
            .cloned()
            .collect()
    }

    /// Whether any entry exists for `feature`.
    pub fn has_feature(&self, feature: &str) -> bool {
        self.entries.iter().any(|e| e.feature == feature)
    }

    /// All entries.
    pub fn entries(&self) -> &[FeatureMapEntry] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_map_filters() {
        let mut m = FeatureMap::new();
        m.add(FeatureMapEntry::new("arPackages", Val::String("p1".into())));
        m.add(FeatureMapEntry::new("adminData", Val::String("d".into())));
        assert_eq!(m.len(), 2);
        assert_eq!(m.list("arPackages").len(), 1);
        assert!(m.has_feature("adminData"));
        m.remove_by_feature("arPackages");
        assert!(!m.has_feature("arPackages"));
        assert_eq!(m.len(), 1);
    }
}
