//! EMF `EList` abstraction and real implementations, ported from C++
//! `emf-common/EList` and `emf-common/util/{AbstractEList,BasicEList,UniqueEList}`.
//!
//! The C++ port uses `std::vector` + virtual methods + a small callback record
//! for notifications (a function pointer `cb_`, a context, and a feature).
//! In Rust we model the list as an owned `Vec<T>` behind the [`EList`] trait,
//! with an optional change hook (`ListChange`) to drive notifications.

/// A change on a list: add / remove / set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListChange {
    /// An element was appended.
    Add { index: usize },
    /// An element was removed.
    Remove { index: usize },
    /// An element was replaced.
    Set { index: usize },
}

/// Generic bounded/ordered list (EMF `EList<T>`).
pub trait EList<T> {
    /// Number of elements.
    fn len(&self) -> usize;
    /// Whether empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Element at index `i`.
    fn get(&self, i: usize) -> Option<&T>;
    /// Append `value`.
    fn add(&mut self, value: T);
    /// Remove element at index `i`, returning it.
    fn remove(&mut self, i: usize) -> Option<T>;
    /// Replace element at index `i`, returning the old value.
    fn set(&mut self, i: usize, value: T) -> Option<T>;
    /// Remove all elements.
    fn clear(&mut self);
    /// Whether `value` is present.
    fn contains(&self, value: &T) -> bool
    where
        T: PartialEq;
    /// Iterate by reference.
    fn iter(&self) -> std::slice::Iter<'_, T>;
}

/// A list backed by `Vec<T>` with no uniqueness constraint (EMF `BasicEList`).
pub struct BasicEList<T> {
    data: Vec<T>,
    /// Optional change hook (aligns to the C++ callback record).
    on_change: Option<Box<dyn FnMut(ListChange)>>,
}

impl<T: std::fmt::Debug> std::fmt::Debug for BasicEList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.data.iter()).finish()
    }
}

impl<T> Default for BasicEList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> Clone for BasicEList<T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            on_change: None,
        }
    }
}

impl<T> BasicEList<T> {
    /// New empty list.
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            on_change: None,
        }
    }

    /// New list seeded from an iterator.
    pub fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self {
            data: iter.into_iter().collect(),
            on_change: None,
        }
    }

    /// Attach a change callback.
    pub fn set_change_hook(&mut self, hook: Option<Box<dyn FnMut(ListChange)>>) {
        self.on_change = hook;
    }

    /// Raw slice access.
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    /// Raw mutable slice access.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data
    }
}

impl<T> EList<T> for BasicEList<T> {
    fn len(&self) -> usize {
        self.data.len()
    }
    fn get(&self, i: usize) -> Option<&T> {
        self.data.get(i)
    }
    fn add(&mut self, value: T) {
        let index = self.data.len();
        self.data.push(value);
        if let Some(h) = self.on_change.as_mut() {
            h(ListChange::Add { index });
        }
    }
    fn remove(&mut self, i: usize) -> Option<T> {
        if i >= self.data.len() {
            return None;
        }
        let v = self.data.remove(i);
        if let Some(h) = self.on_change.as_mut() {
            h(ListChange::Remove { index: i });
        }
        Some(v)
    }
    fn set(&mut self, i: usize, value: T) -> Option<T> {
        if i >= self.data.len() {
            return None;
        }
        let old = std::mem::replace(&mut self.data[i], value);
        if let Some(h) = self.on_change.as_mut() {
            h(ListChange::Set { index: i });
        }
        Some(old)
    }
    fn clear(&mut self) {
        if !self.data.is_empty() {
            self.data.clear();
        }
    }
    fn contains(&self, value: &T) -> bool
    where
        T: PartialEq,
    {
        self.data.contains(value)
    }
    fn iter(&self) -> std::slice::Iter<'_, T> {
        self.data.iter()
    }
}

impl<T: PartialEq> BasicEList<T> {
    /// Add only if not already present (EMF `UniqueEList.addUnique`).
    pub fn add_unique(&mut self, value: T) -> bool {
        if self.data.contains(&value) {
            false
        } else {
            let index = self.data.len();
            self.data.push(value);
            if let Some(h) = self.on_change.as_mut() {
                h(ListChange::Add { index });
            }
            true
        }
    }

    /// Position of `value`, else `None`.
    pub fn index_of(&self, value: &T) -> Option<usize> {
        self.data.iter().position(|e| e == value)
    }

    /// Remove first occurrence of `value`.
    pub fn remove_value(&mut self, value: &T) -> bool {
        if let Some(i) = self.index_of(value) {
            self.remove(i);
            true
        } else {
            false
        }
    }
}

impl<T> IntoIterator for BasicEList<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

/// `UniqueEList<T>` guaranteeing no duplicate elements.
#[derive(Debug, Clone, Default)]
pub struct UniqueEList<T> {
    inner: BasicEList<T>,
}

impl<T> UniqueEList<T> {
    pub fn new() -> Self {
        Self {
            inner: BasicEList::new(),
        }
    }
}

impl<T> EList<T> for UniqueEList<T>
where
    T: PartialEq,
{
    fn len(&self) -> usize {
        self.inner.len()
    }
    fn get(&self, i: usize) -> Option<&T> {
        self.inner.get(i)
    }
    fn add(&mut self, value: T) {
        self.inner.add_unique(value);
    }
    fn remove(&mut self, i: usize) -> Option<T> {
        self.inner.remove(i)
    }
    fn set(&mut self, i: usize, value: T) -> Option<T> {
        self.inner.set(i, value)
    }
    fn clear(&mut self) {
        self.inner.clear();
    }
    fn contains(&self, value: &T) -> bool {
        self.inner.contains(value)
    }
    fn iter(&self) -> std::slice::Iter<'_, T> {
        self.inner.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn basic_list_ops() {
        let mut l = BasicEList::new();
        l.add(1);
        l.add(2);
        l.add(3);
        assert_eq!(l.len(), 3);
        assert_eq!(l.get(1), Some(&2));
        assert_eq!(l.remove(0), Some(1));
        assert_eq!(l.len(), 2);
        assert!(l.contains(&3));
    }

    #[test]
    fn change_hook_fires() {
        let mut l = BasicEList::new();
        let events: Rc<RefCell<Vec<ListChange>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&events);
        l.set_change_hook(Some(Box::new(move |e| sink.borrow_mut().push(e))));
        l.add(7);
        l.remove(0);
        assert_eq!(
            *events.borrow(),
            vec![
                ListChange::Add { index: 0 },
                ListChange::Remove { index: 0 },
            ]
        );
    }

    #[test]
    fn unique_add_deduplicates() {
        let mut l = BasicEList::new();
        l.add_unique(1);
        assert!(!l.add_unique(1));
        assert_eq!(l.len(), 1);
    }
}
