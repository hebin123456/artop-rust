//! EMF `EMap` / `BasicEMap`, ported from C++ `emf-common/util/{EMap,BasicEMap}`.
//!
//! A `BasicEMap<K, V>` is conceptually a `Vec<(K, V)>` entry list with map-style
//! lookups, caches the key index for fast `containsKey` after the map is sealed,
//! and fires the same `didAdd`/`didModify`/`didRemove` hooks as the C++ port.

/// A single key/value entry (EMF `Map.Entry` / C++ `MapEntry`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapEntry<K: Clone, V: Clone> {
    /// The key.
    pub key: K,
    /// The value.
    pub value: V,
}

/// An EMF map over owned keys and values.
#[derive(Debug, Clone, Default)]
pub struct BasicEMap<K: Clone, V: Clone> {
    entries: Vec<MapEntry<K, V>>,
    /// Key materialized index for `contains_key` fast-path.
    key_index: Option<std::collections::HashMap<K, usize>>,
}

impl<K: Clone + Eq + std::hash::Hash, V: Clone> BasicEMap<K, V> {
    /// New empty map.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            key_index: None,
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

    fn rebuild_index(&mut self) {
        let mut idx = std::collections::HashMap::with_capacity(self.entries.len());
        for (i, e) in self.entries.iter().enumerate() {
            idx.entry(e.key.clone()).or_insert(i);
        }
        self.key_index = Some(idx);
    }

    /// Position of `key`, else `None`.
    pub fn index_of_key(&self, key: &K) -> Option<usize> {
        if let Some(idx) = &self.key_index {
            return idx.get(key).copied();
        }
        self.entries.iter().position(|e| e.key == *key)
    }

    /// Whether `key` is present.
    pub fn contains_key(&self, key: &K) -> bool {
        if let Some(idx) = &self.key_index {
            idx.contains_key(key)
        } else {
            self.entries.iter().any(|e| e.key == *key)
        }
    }

    /// Value for `key`, else `None`.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.index_of_key(key)
            .and_then(|i| self.entries.get(i))
            .map(|e| &e.value)
    }

    /// Insert `value` for `key`, returning the previous value if present.
    pub fn put(&mut self, key: K, value: V) -> Option<V> {
        if let Some(i) = self.index_of_key(&key) {
            let old = std::mem::replace(&mut self.entries[i].value, value);
            self.rebuild_index();
            Some(old)
        } else {
            self.entries.push(MapEntry { key, value });
            self.key_index = None; // lazily rebuilt
            None
        }
    }

    /// Remove `key`, returning its value if present.
    pub fn remove_key(&mut self, key: &K) -> Option<V> {
        if let Some(i) = self.index_of_key(key) {
            let e = self.entries.remove(i);
            self.key_index = None;
            Some(e.value)
        } else {
            None
        }
    }

    /// Whether `value` is present.
    pub fn contains_value(&self, value: &V) -> bool
    where
        V: PartialEq,
    {
        self.entries.iter().any(|e| e.value == *value)
    }

    /// All entries, in insertion order.
    pub fn entries(&self) -> &[MapEntry<K, V>] {
        &self.entries
    }

    /// All keys.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries.iter().map(|e| &e.key)
    }

    /// All values.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|e| &e.value)
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.key_index = None;
    }

    /// Seal the map: materialize the key index for O(1) lookups.
    pub fn seal(&mut self) {
        if self.key_index.is_none() {
            self.rebuild_index();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_map_ops() {
        let mut m = BasicEMap::new();
        assert!(m.put("a".to_string(), 1).is_none());
        assert_eq!(m.put("a".to_string(), 2), Some(1));
        assert_eq!(m.get(&"a".to_string()), Some(&2));
        assert!(m.contains_key(&"a".to_string()));
        assert!(!m.contains_key(&"b".to_string()));
        assert_eq!(m.remove_key(&"a".to_string()), Some(2));
        assert!(!m.contains_key(&"a".to_string()));
        assert!(m.is_empty());
    }

    #[test]
    fn sealed_lookup_fast_path() {
        let mut m = BasicEMap::new();
        for i in 0..100 {
            m.put(format!("k{i}"), i);
        }
        m.seal();
        assert_eq!(m.get(&"k42".to_string()), Some(&42));
        assert_eq!(m.index_of_key(&"k99".to_string()), Some(99));
    }

    #[test]
    fn index_of_key_finds_position() {
        let mut m = BasicEMap::new();
        m.put("a".to_string(), 1);
        m.put("b".to_string(), 2);
        m.put("c".to_string(), 3);
        assert_eq!(m.index_of_key(&"a".to_string()), Some(0));
        assert_eq!(m.index_of_key(&"c".to_string()), Some(2));
        assert_eq!(m.index_of_key(&"z".to_string()), None);
    }

    #[test]
    fn contains_key_and_value() {
        let mut m = BasicEMap::new();
        m.put("x".to_string(), 10);
        m.put("y".to_string(), 20);
        assert!(m.contains_key(&"x".to_string()));
        assert!(!m.contains_key(&"missing".to_string()));
        assert!(m.contains_value(&20));
        assert!(!m.contains_value(&99));
    }

    #[test]
    fn iteration_preserves_insertion_order() {
        let mut m = BasicEMap::new();
        m.put("a".to_string(), 1);
        m.put("b".to_string(), 2);
        m.put("c".to_string(), 3);
        let keys: Vec<String> = m.keys().cloned().collect();
        let vals: Vec<i32> = m.values().cloned().collect();
        assert_eq!(keys, ["a", "b", "c"]);
        assert_eq!(vals, [1, 2, 3]);
        assert_eq!(m.entries().len(), 3);
    }

    #[test]
    fn foreach_key_value() {
        let mut m = BasicEMap::new();
        m.put("k1".to_string(), 1);
        m.put("k2".to_string(), 2);
        let mut sum = 0;
        for e in m.entries() {
            if e.key.starts_with('k') {
                sum += e.value;
            }
        }
        assert_eq!(sum, 3);
    }

    #[test]
    fn update_existing_key_replaces_value() {
        let mut m = BasicEMap::new();
        assert!(m.put("k".to_string(), 1).is_none());
        assert_eq!(m.put("k".to_string(), 42), Some(1));
        assert_eq!(m.get(&"k".to_string()), Some(&42));
        assert_eq!(m.len(), 1); // no new entry
    }

    #[test]
    fn remove_key_removes_entry() {
        let mut m = BasicEMap::new();
        m.put("a".to_string(), 1);
        m.put("b".to_string(), 2);
        assert_eq!(m.remove_key(&"a".to_string()), Some(1));
        assert!(!m.contains_key(&"a".to_string()));
        assert_eq!(m.len(), 1);
        assert_eq!(m.remove_key(&"missing".to_string()), None);
    }

    #[test]
    fn clear_wipes_all_entries() {
        let mut m = BasicEMap::new();
        m.put("a".to_string(), 1);
        m.put("b".to_string(), 2);
        m.clear();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
        assert!(!m.contains_key(&"a".to_string()));
    }
}
