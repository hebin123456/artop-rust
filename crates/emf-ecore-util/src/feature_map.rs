//! `FeatureMap` — an ordered list of `(feature, value)` entries.
//!
//! Port of C++ `emf-ecore-util/FeatureMap` + `BasicFeatureMap` (aligned to Java
//! `org.eclipse.emf.ecore.util.FeatureMap`).
//!
//! A feature map is used to hold heterogeneous groups of features (XML
//! `group` / `choice` / `sequence` / `anyAttribute`) in document order. Each
//! entry binds a [`EStructuralFeature`] to a [`Val`]. The map supports both a
//! flat query (`entries(feature)` / `size(feature)`) and an append-oriented
//! API. Everything is generic EMF — no domain metamodel knowledge (see the
//! decoupling principle in `docs/PROGRESS.md`).

use emf_common::value::Val;
use emf_ecore::EStructuralFeature;

/// One `(feature, value)` pair in a [`FeatureMap`].
#[derive(Debug, Clone)]
pub struct Entry {
    feature: EStructuralFeature,
    value: Val,
}

impl Entry {
    /// New entry.
    pub fn new(feature: EStructuralFeature, value: Val) -> Self {
        Self { feature, value }
    }

    /// The structural feature.
    pub fn feature(&self) -> &EStructuralFeature {
        &self.feature
    }

    /// The feature's integer id (`-1` if unassigned).
    pub fn feature_id(&self) -> i32 {
        self.feature.feature_id()
    }

    /// The bound value.
    pub fn value(&self) -> &Val {
        &self.value
    }

    /// The bound value, mutably.
    pub fn value_mut(&mut self) -> &mut Val {
        &mut self.value
    }

    /// Replace the value, returning the old one.
    pub fn set_value(&mut self, value: Val) -> Val {
        std::mem::replace(&mut self.value, value)
    }
}

/// An ordered `(feature, value)` entry list (C++ `FeatureMap` /
/// `BasicFeatureMap`).
#[derive(Debug, Clone, Default)]
pub struct FeatureMap {
    entries: Vec<Entry>,
}

impl FeatureMap {
    /// An empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// All entries, in order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The entry at `index`, if any.
    pub fn get(&self, index: usize) -> Option<&Entry> {
        self.entries.get(index)
    }

    /// Append an entry.
    pub fn add(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    /// Append a `(feature, value)` pair.
    pub fn add_entry(&mut self, feature: EStructuralFeature, value: Val) {
        self.entries.push(Entry::new(feature, value));
    }

    /// Entries whose feature id matches `feature_id`.
    pub fn entries_for(&self, feature: &EStructuralFeature) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.feature().feature_id() == feature.feature_id())
            .collect()
    }

    /// Number of entries for the given feature.
    pub fn size_for(&self, feature: &EStructuralFeature) -> usize {
        self.entries_for(feature).len()
    }

    /// The entry at `index` *within* the given feature's slice.
    pub fn get_for(&self, feature: &EStructuralFeature, index: usize) -> Option<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.feature().feature_id() == feature.feature_id())
            .nth(index)
    }

    /// Set the value of the `index`-th entry for `feature`, returning the old.
    pub fn set_for(
        &mut self,
        feature: &EStructuralFeature,
        index: usize,
        value: Val,
    ) -> Option<Val> {
        let mut idx = None;
        let mut count = 0usize;
        for (i, e) in self.entries.iter().enumerate() {
            if e.feature().feature_id() == feature.feature_id() {
                if count == index {
                    idx = Some(i);
                    break;
                }
                count += 1;
            }
        }
        idx.map(|i| self.entries[i].set_value(value))
    }

    /// Remove the entry at `index`, returning it.
    pub fn remove(&mut self, index: usize) -> Option<Entry> {
        if index < self.entries.len() {
            Some(self.entries.remove(index))
        } else {
            None
        }
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl FromIterator<Entry> for FeatureMap {
    fn from_iter<T: IntoIterator<Item = Entry>>(iter: T) -> Self {
        Self {
            entries: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_ecore::EStructuralFeature;

    #[test]
    fn appends_and_queries_by_feature() {
        let mut a = EStructuralFeature::attribute("a");
        a.set_feature_id(0);
        let mut b = EStructuralFeature::attribute("b");
        b.set_feature_id(1);

        let mut f = FeatureMap::new();
        f.add_entry(a.clone(), Val::Int(1));
        f.add_entry(b.clone(), Val::Int(2));
        f.add_entry(a.clone(), Val::Int(3));

        assert_eq!(f.len(), 3);
        assert_eq!(f.size_for(&a), 2);
        assert_eq!(f.size_for(&b), 1);
        let e = f.get_for(&a, 1).unwrap();
        assert_eq!(e.value(), &Val::Int(3));
    }

    #[test]
    fn set_for_replaces_value() {
        let mut a = EStructuralFeature::attribute("a");
        a.set_feature_id(0);
        let mut f = FeatureMap::new();
        f.add_entry(a.clone(), Val::Int(1));
        let old = f.set_for(&a, 0, Val::Int(99)).unwrap();
        assert_eq!(old, Val::Int(1));
        assert_eq!(f.get_for(&a, 0).unwrap().value(), &Val::Int(99));
    }

    #[test]
    fn preserves_document_order() {
        let mut a = EStructuralFeature::attribute("a");
        a.set_feature_id(0);
        let mut b = EStructuralFeature::attribute("b");
        b.set_feature_id(1);
        // Even a feature's entries keep global document order, not group order:
        // iterate globally and filter, which is what EMF serialization does.
        let mut f = FeatureMap::new();
        f.add_entry(a.clone(), Val::Int(1));
        f.add_entry(b.clone(), Val::Int(2));
        f.add_entry(a.clone(), Val::Int(3));
        let a_vals: Vec<_> = f
            .entries_for(&a)
            .iter()
            .map(|e| e.value().clone())
            .collect();
        assert_eq!(a_vals, vec![Val::Int(1), Val::Int(3)]);
    }
}
