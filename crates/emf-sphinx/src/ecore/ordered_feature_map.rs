//! `OrderedFeatureMap` — feature-map entries kept sorted by feature ID.
//!
//! Port of C++ `emf/sphinx/ecore/OrderedFeatureMap.h` (aligned to Java
//! `org.eclipse.sphinx.emf.ecore.OrderedFeatureMap`).

use std::rc::Rc;

use emf_common::value::ObjectRef;
use emf_ecore::EStructuralFeature;

/// One entry: `(feature, value, index)`.
#[derive(Debug, Clone)]
pub struct FeatureMapEntry {
    /// The feature this entry belongs to (`None` sorts first, as order `-1`).
    pub feature: Option<Rc<EStructuralFeature>>,
    /// The value (aligned to Java `FeatureMap.Entry.value`).
    pub value: Option<ObjectRef>,
    /// Position within the same feature.
    pub index: i32,
}

impl FeatureMapEntry {
    /// Construct an entry.
    pub fn new(
        feature: Option<Rc<EStructuralFeature>>,
        value: Option<ObjectRef>,
        index: i32,
    ) -> Self {
        Self {
            feature,
            value,
            index,
        }
    }
}

impl Default for FeatureMapEntry {
    fn default() -> Self {
        Self {
            feature: None,
            value: None,
            index: -1,
        }
    }
}

/// Feature-map entries ordered by feature ID (then by index).
#[derive(Debug, Clone, Default)]
pub struct OrderedFeatureMap {
    entries: Vec<FeatureMapEntry>,
}

impl OrderedFeatureMap {
    /// A new empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// The default ordering key: the feature ID, or `-1` when the feature is
    /// absent (aligned to C++ `defaultOrder`).
    pub fn default_order(entry: &FeatureMapEntry) -> i32 {
        entry.feature.as_ref().map(|f| f.feature_id()).unwrap_or(-1)
    }

    /// Insert an entry, keeping entries sorted by order then index.
    pub fn add(
        &mut self,
        feature: Option<Rc<EStructuralFeature>>,
        value: Option<ObjectRef>,
        index: i32,
    ) {
        let entry = FeatureMapEntry::new(feature, value, index);
        let order = Self::default_order(&entry);
        let pos = self
            .entries
            .iter()
            .position(|other| {
                let other_order = Self::default_order(other);
                if order != other_order {
                    other_order > order
                } else {
                    other.index > index
                }
            })
            .unwrap_or(self.entries.len());
        self.entries.insert(pos, entry);
    }

    /// Number of entries.
    pub fn size(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// All entries in order.
    pub fn entries(&self) -> &[FeatureMapEntry] {
        &self.entries
    }

    /// Remove every entry.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Entries belonging to `feature` (compared by handle identity).
    pub fn get(&self, feature: &Rc<EStructuralFeature>) -> Vec<FeatureMapEntry> {
        self.entries
            .iter()
            .filter(|e| e.feature.as_ref().is_some_and(|f| Rc::ptr_eq(f, feature)))
            .cloned()
            .collect()
    }
}
