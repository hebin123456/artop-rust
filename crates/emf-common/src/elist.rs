//! EMF `EList` abstraction and real implementations, ported from C++
//! `emf-common/EList` and `emf-common/util/{AbstractEList,BasicEList,UniqueEList}`.
//!
//! The C++ port uses `std::vector` + virtual methods + a small callback record
//! for notifications (a function pointer `cb_`, a context, and a feature).
//! In Rust we model the list as an owned `Vec<T>` behind the [`EList`] trait,
//! with an optional change hook (`ListChange`) to drive notifications.

use crate::notification::{EventType, Notification, Notifier};
use crate::value::Val;
use std::cell::RefCell;
use std::rc::Rc;

/// A change on a list: add / remove / set / move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListChange {
    /// An element was appended.
    Add { index: usize },
    /// An element was removed.
    Remove { index: usize },
    /// An element was replaced.
    Set { index: usize },
    /// An element was moved.
    Move { from: usize, to: usize },
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

    /// Move the element currently at `object_index` to `new_position`, returning
    /// the moved value (EMF `EList.move(newPosition, object)`; the C++ port
    /// treats the second argument as an index). Out-of-range indices return
    /// `None`.
    pub fn move_element(&mut self, new_position: usize, object_index: usize) -> Option<T>
    where
        T: Clone,
    {
        if object_index >= self.data.len() || new_position >= self.data.len() {
            return None;
        }
        let v = self.data.remove(object_index);
        let mut target = new_position;
        // removal shifts later elements left by one.
        if target > object_index {
            target -= 1;
        }
        self.data.insert(target, v);
        if let Some(h) = self.on_change.as_mut() {
            h(ListChange::Move {
                from: object_index,
                to: target,
            });
        }
        Some(self.data[target].clone())
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

    /// New list with the given reserved capacity (EMF `UniqueEList(int)`).
    pub fn with_capacity(capacity: usize) -> Self {
        let mut inner = BasicEList::new();
        inner.data = Vec::with_capacity(capacity);
        Self { inner }
    }

    /// Build from an iterator, dropping duplicates (EMF `UniqueEList` ctor
    /// from a collection).
    pub fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self
    where
        T: PartialEq,
    {
        let mut l = UniqueEList::new();
        for v in iter {
            l.add(v);
        }
        l
    }

    /// A read-only handle to the underlying [`BasicEList`]. In C++ `UniqueEList`
    /// *is a* `BasicEList` (inheritance); Rust models this with composition, so
    /// this exposes the same "view as a BasicEList" capability.
    pub fn as_basic_elist(&self) -> &BasicEList<T> {
        &self.inner
    }
}

/// Distinct error kinds for the uniqueness-constrained operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniqueDuplicateError {
    /// The value is already present elsewhere in the list.
    Duplicate,
    /// The index is out of range.
    OutOfRange,
}

impl<T: PartialEq> UniqueEList<T> {
    /// Insert `value` at `index`, refusing duplicates (EMF `UniqueEList.add`
    /// at a specific index throws `invalid_argument` on a duplicate).
    pub fn try_add_at_index(&mut self, index: usize, value: T) -> Result<(), UniqueDuplicateError> {
        if self.inner.data.iter().any(|e| e == &value) {
            return Err(UniqueDuplicateError::Duplicate);
        }
        if index > self.inner.data.len() {
            return Err(UniqueDuplicateError::OutOfRange);
        }
        self.inner.data.insert(index, value);
        if let Some(h) = self.inner.on_change.as_mut() {
            h(ListChange::Add { index });
        }
        Ok(())
    }

    /// Replace the element at `index` with `value`, refusing duplicates (EMF
    /// `UniqueEList.set` throws `invalid_argument` when `value` is already
    /// present at another index).
    pub fn try_set(&mut self, index: usize, value: T) -> Result<Option<T>, UniqueDuplicateError> {
        if index >= self.inner.data.len() {
            return Err(UniqueDuplicateError::OutOfRange);
        }
        if self
            .inner
            .data
            .iter()
            .enumerate()
            .any(|(i, e)| i != index && e == &value)
        {
            return Err(UniqueDuplicateError::Duplicate);
        }
        let old = std::mem::replace(&mut self.inner.data[index], value);
        if let Some(h) = self.inner.on_change.as_mut() {
            h(ListChange::Set { index });
        }
        Ok(Some(old))
    }

    /// Move the element at `object_index` to `new_position` (delegates to the
    /// underlying basic list's move semantics).
    pub fn move_element(&mut self, new_position: usize, object_index: usize) -> Option<T>
    where
        T: Clone,
    {
        self.inner.move_element(new_position, object_index)
    }
}

/// A uniqueness-constrained list whose duplicate test is *identity-only*
/// (the C++ `FastCompareUniqueEList`). For the primitive/atomic case exposed
/// in the reference tests this degenerates to value equality, matching the
/// observed C++ behavior for `int`.
#[derive(Debug, Clone, Default)]
pub struct FastCompareUniqueEList<T: PartialEq>(UniqueEList<T>);

impl<T: PartialEq> FastCompareUniqueEList<T> {
    pub fn new() -> Self {
        Self(UniqueEList::new())
    }

    /// Add `value`, returning `false` if it is already present (matching
    /// `UniqueEList.addUnique` semantics for the identity-only variant).
    pub fn add(&mut self, value: T) -> bool {
        if self.0.contains(&value) {
            return false;
        }
        self.0.add(value);
        true
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
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

/// A list that dispatches EMF-style change notifications to a [`Notifier`]
/// whenever it is mutated (ported from C++ `NotifyingListImpl`). An *active*
/// list (built with [`NotifyingList::with_notifier`]) fires notifications to
/// the notifier's adapters; a *passive* default list never notifies.
pub struct NotifyingList<T> {
    data: Vec<T>,
    notifier: Option<Rc<RefCell<Notifier>>>,
    feature_id: i32,
    required: bool,
}

impl<T: Clone + Into<Val>> NotifyingList<T> {
    /// A passive list: no notifier, notifications not required.
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            notifier: None,
            feature_id: -1,
            required: false,
        }
    }

    /// An active list wired up to the given notifier (notifications required).
    pub fn with_notifier(notifier: Rc<RefCell<Notifier>>) -> Self {
        Self {
            data: Vec::new(),
            notifier: Some(notifier),
            feature_id: 42,
            required: true,
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn get(&self, i: usize) -> Option<&T> {
        self.data.get(i)
    }

    /// Whether there is no notifier attached (default lists report `None`).
    pub fn get_notifier_is_null(&self) -> bool {
        self.notifier.is_none()
    }

    /// Feature id reported on the notifications (defaults to `-1`).
    pub fn get_feature_id(&self) -> i32 {
        self.feature_id
    }

    pub fn contains(&self, value: &T) -> bool
    where
        T: PartialEq,
    {
        self.data.contains(value)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.data.iter()
    }

    /// Dispatch a change to the notifier iff notifications are required.
    fn dispatch(&self, event: EventType, old: Val, new: Val, position: i32) {
        if !self.required {
            return;
        }
        if let Some(n) = &self.notifier {
            let nt = Notification::new(event, None, old, new, position, false);
            n.borrow_mut().e_notify(&nt);
        }
    }

    pub fn add(&mut self, value: T) {
        let index = self.data.len() as i32;
        let v: Val = value.clone().into();
        self.data.push(value);
        self.dispatch(EventType::Add, Val::Null, v, index);
    }

    pub fn add_at_index(&mut self, index: usize, value: T) {
        let v: Val = value.clone().into();
        self.data.insert(index, value);
        self.dispatch(EventType::Add, Val::Null, v, index as i32);
    }

    /// Append many elements in one go; a single element yields an `ADD`
    /// notification while multiple yield `ADD_MANY`.
    pub fn add_all(&mut self, values: Vec<T>) {
        let start = self.data.len() as i32;
        for v in &values {
            self.data.push(v.clone());
        }
        if values.len() == 1 {
            let single: Val = values[0].clone().into();
            self.dispatch(EventType::Add, Val::Null, single, start);
        } else if values.len() > 1 {
            let list: Vec<Val> = values.iter().cloned().map(Into::into).collect();
            self.dispatch(EventType::AddMany, Val::Null, Val::List(list), start);
        }
    }

    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.data.len() {
            return None;
        }
        let v = self.data.remove(index);
        let old: Val = v.clone().into();
        self.dispatch(EventType::Remove, old, Val::Null, index as i32);
        Some(v)
    }

    pub fn set(&mut self, index: usize, value: T) -> Option<T> {
        if index >= self.data.len() {
            return None;
        }
        let old = std::mem::replace(&mut self.data[index], value);
        let ov: Val = old.clone().into();
        let nv: Val = self.data[index].clone().into();
        self.dispatch(EventType::Set, ov, nv, index as i32);
        Some(old)
    }

    pub fn move_element(&mut self, new_position: usize, object_index: usize) -> Option<T> {
        if object_index >= self.data.len() || new_position >= self.data.len() {
            return None;
        }
        let v = self.data.remove(object_index);
        let mut target = new_position;
        if target > object_index {
            target -= 1;
        }
        self.data.insert(target, v);
        let nv: Val = self.data[target].clone().into();
        self.dispatch(EventType::Move, Val::Null, nv, new_position as i32);
        Some(self.data[target].clone())
    }

    pub fn clear(&mut self) {
        if self.data.is_empty() {
            return;
        }
        let old: Vec<Val> = self.data.iter().cloned().map(Into::into).collect();
        self.data.clear();
        self.dispatch(EventType::RemoveMany, Val::List(old), Val::Null, 0);
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

    #[test]
    fn operator_bracket_get_by_index() {
        let mut l = BasicEList::new();
        l.add(10);
        l.add(20);
        l.add(30);
        assert_eq!(l.get(0), Some(&10));
        assert_eq!(l.get(2), Some(&30));
        // Out of range yields None (the Rust port models "throws" as `None`).
        assert_eq!(l.get(3), None);
    }

    #[test]
    fn set_replaces_element() {
        let mut l = BasicEList::new();
        l.add(1);
        l.add(2);
        l.add(3);
        assert_eq!(l.set(1, 99), Some(2));
        assert_eq!(l.get(1), Some(&99));
        assert_eq!(l.len(), 3); // set does not change size
        assert_eq!(l.set(5, 0), None); // out of range
    }

    #[test]
    fn remove_by_index_and_value() {
        let mut l = BasicEList::new();
        l.add(1);
        l.add(2);
        l.add(1);
        assert_eq!(l.remove(0), Some(1)); // by index
        assert!(l.remove_value(&1)); // removes first occurrence by value
        assert_eq!(l.as_slice(), [2]);
        assert!(!l.remove_value(&1)); // no further occurrence
        assert_eq!(l.as_slice(), [2]);
        assert!(!l.remove_value(&999)); // absent
    }

    #[test]
    fn clear_empties_list() {
        let mut l = BasicEList::new();
        l.add(1);
        l.add(2);
        l.clear();
        assert!(l.is_empty());
        assert_eq!(l.len(), 0);
        l.clear(); // clearing an already-empty list is a no-op
        assert!(l.is_empty());
    }

    #[test]
    fn iteration_yields_elements_in_order() {
        let mut l = BasicEList::new();
        l.add(5);
        l.add(6);
        l.add(7);
        let collected: Vec<i32> = l.iter().copied().collect();
        assert_eq!(collected, [5, 6, 7]);
    }

    #[test]
    fn contains_and_index_of() {
        let mut l = BasicEList::new();
        l.add(1);
        l.add(2);
        l.add(3);
        assert!(l.contains(&2));
        assert!(!l.contains(&9));
        assert_eq!(l.index_of(&2), Some(1));
        assert_eq!(l.index_of(&9), None);
    }

    #[test]
    fn unique_elist_add_deduplicates() {
        let mut l: UniqueEList<i32> = UniqueEList::new();
        l.add(1);
        l.add(2);
        l.add(1); // duplicate ignored
        assert_eq!(l.len(), 2);
        assert_eq!(l.get(0), Some(&1));
        assert_eq!(l.get(1), Some(&2));
        // Add-all style: from an iterator with duplicates drops duplicates.
        let from = UniqueEList::from_iter(vec![3, 4, 3, 5]);
        assert_eq!(from.len(), 3);
    }

    #[test]
    fn unique_elist_remove_and_clear() {
        let mut l: UniqueEList<i32> = UniqueEList::new();
        l.add(1);
        l.add(2);
        l.add(3);
        assert_eq!(l.remove(1), Some(2));
        assert_eq!(l.len(), 2);
        assert!(l.contains(&3));
        l.clear();
        assert!(l.is_empty());
    }

    // ---- UniqueEList: required ports ----

    #[test]
    fn unique_elist_add_at_index_duplicate_throws() {
        let mut list: UniqueEList<i32> = UniqueEList::new();
        list.add(10);
        list.add(20);
        assert_eq!(list.try_add_at_index(1, 20), Err(UniqueDuplicateError::Duplicate));
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn unique_elist_constructor_from_collection() {
        let list = UniqueEList::from_iter([1, 2, 2, 3, 1, 4]);
        assert_eq!(list.len(), 4);
    }

    #[test]
    fn unique_elist_constructor_with_capacity() {
        let mut list: UniqueEList<i32> = UniqueEList::with_capacity(100);
        assert_eq!(list.len(), 0);
        for i in 0..50 {
            list.add(i);
        }
        assert_eq!(list.len(), 50);
    }

    #[test]
    fn unique_elist_fast_compare_identity_only() {
        let mut list: FastCompareUniqueEList<i32> = FastCompareUniqueEList::new();
        let a: i32 = 42;
        let b: i32 = 42;
        assert!(list.add(a));
        assert!(!list.add(b)); // value-equal duplicate rejected
        assert_eq!(list.len(), 1);
        assert!(list.add(100));
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn unique_elist_is_basic_elist_subclass() {
        let list: UniqueEList<i32> = UniqueEList::new();
        // C++ does a static_assert(is_base_of<BasicEList,UniqueEList>); Rust
        // replaces inheritance with composition, verified via the BasicEList
        // view.
        let base = list.as_basic_elist();
        assert_eq!(base.len(), 0);
    }

    #[test]
    fn unique_elist_move() {
        let mut list: UniqueEList<i32> = UniqueEList::new();
        list.add(10);
        list.add(20);
        list.add(30);
        list.add(40);
        let moved = list.move_element(0, 2); // 30 -> index 0
        assert_eq!(moved, Some(30));
        assert_eq!(list.get(0), Some(&30));
        assert_eq!(list.get(1), Some(&10));
        assert_eq!(list.get(2), Some(&20));
        assert_eq!(list.get(3), Some(&40));
    }

    #[test]
    fn unique_elist_set_duplicate_throws() {
        let mut list: UniqueEList<i32> = UniqueEList::new();
        list.add(1);
        list.add(2);
        assert_eq!(list.try_set(1, 1), Err(UniqueDuplicateError::Duplicate));
        assert_eq!(list.get(1), Some(&2));
    }

    // ---- NotifyingList: required ports ----
    use crate::notification::Adapter as _Adapter;
    use crate::notification::EventType;

    struct RecordingCb {
        events: Rc<RefCell<Vec<Notification>>>,
    }
    impl _Adapter for RecordingCb {
        fn notify_changed(&mut self, notification: &Notification) {
            self.events.borrow_mut().push(notification.clone());
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    /// Build an active notifier with one recording adapter, returning the
    /// notifier handle and the shared event buffer.
    fn recording_env() -> (Rc<RefCell<Notifier>>, Rc<RefCell<Vec<Notification>>>) {
        let n = Rc::new(RefCell::new(Notifier::new()));
        let events = Rc::new(RefCell::new(Vec::new()));
        n.borrow_mut()
            .add_adapter(Box::new(RecordingCb { events: Rc::clone(&events) }));
        (n, events)
    }

    #[test]
    fn notifying_basic_add_remove() {
        let mut list: NotifyingList<i32> = NotifyingList::new();
        list.add(1);
        list.add(2);
        list.add(3);
        assert_eq!(list.len(), 3);
        assert_eq!(list.get(0), Some(&1));
        assert_eq!(list.get(2), Some(&3));
        assert_eq!(list.remove(1), Some(2));
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn notifying_default_get_notifier_is_null() {
        let list: NotifyingList<i32> = NotifyingList::new();
        assert!(list.get_notifier_is_null());
        assert_eq!(list.get_feature_id(), -1);
    }

    #[test]
    fn notifying_dispatch_add() {
        let (n, ev) = recording_env();
        let mut list: NotifyingList<i32> = NotifyingList::with_notifier(n);
        list.add(100);
        let rec = ev.borrow();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].event(), EventType::Add);
        assert_eq!(rec[0].position, 0);
        assert_eq!(rec[0].new_value, Val::Int(100));
    }

    #[test]
    fn notifying_dispatch_add_at_index() {
        let (n, ev) = recording_env();
        let mut list: NotifyingList<i32> = NotifyingList::with_notifier(n);
        list.add(1);
        list.add(2);
        ev.borrow_mut().clear();
        list.add_at_index(1, 99);
        let rec = ev.borrow();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].event(), EventType::Add);
        assert_eq!(rec[0].position, 1);
        assert_eq!(rec[0].new_value, Val::Int(99));
    }

    #[test]
    fn notifying_dispatch_remove() {
        let (n, ev) = recording_env();
        let mut list: NotifyingList<String> = NotifyingList::with_notifier(n);
        list.add(String::from("hello"));
        list.add(String::from("world"));
        ev.borrow_mut().clear();
        assert_eq!(list.remove(0), Some(String::from("hello")));
        let rec = ev.borrow();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].event(), EventType::Remove);
        assert_eq!(rec[0].position, 0);
        assert_eq!(rec[0].old_value, Val::String("hello".into()));
    }

    #[test]
    fn notifying_dispatch_set() {
        let (n, ev) = recording_env();
        let mut list: NotifyingList<i32> = NotifyingList::with_notifier(n);
        list.add(1);
        list.add(2);
        ev.borrow_mut().clear();
        assert_eq!(list.set(1, 200), Some(2));
        let rec = ev.borrow();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].event(), EventType::Set);
        assert_eq!(rec[0].position, 1);
        assert_eq!(rec[0].old_value, Val::Int(2));
        assert_eq!(rec[0].new_value, Val::Int(200));
    }

    #[test]
    fn notifying_dispatch_move() {
        let (n, ev) = recording_env();
        let mut list: NotifyingList<i32> = NotifyingList::with_notifier(n);
        list.add(10);
        list.add(20);
        list.add(30);
        ev.borrow_mut().clear();
        assert_eq!(list.move_element(0, 2), Some(30));
        let rec = ev.borrow();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].event(), EventType::Move);
        assert_eq!(rec[0].position, 0);
        assert_eq!(rec[0].new_value, Val::Int(30));
    }

    #[test]
    fn notifying_dispatch_clear() {
        let (n, ev) = recording_env();
        let mut list: NotifyingList<i32> = NotifyingList::with_notifier(n);
        list.add(1);
        list.add(2);
        list.add(3);
        ev.borrow_mut().clear();
        list.clear();
        assert!(list.is_empty());
        let rec = ev.borrow();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].event(), EventType::RemoveMany);
    }

    #[test]
    fn notifying_no_notification_when_not_required() {
        let (_n, ev) = recording_env();
        let mut list: NotifyingList<i32> = NotifyingList::new(); // passive
        list.add(1);
        list.add(2);
        list.add(3);
        list.remove(0);
        list.set(0, 99);
        list.clear();
        assert_eq!(ev.borrow().len(), 0);
    }

    #[test]
    fn notifying_dispatch_add_all_many() {
        let (n, ev) = recording_env();
        let mut list: NotifyingList<i32> = NotifyingList::with_notifier(n);
        list.add_all(vec![7, 8, 9]);
        let rec = ev.borrow();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].event(), EventType::AddMany);
        assert_eq!(rec[0].position, 0);
        match &rec[0].new_value {
            Val::List(l) => {
                assert_eq!(l, &vec![Val::Int(7), Val::Int(8), Val::Int(9)]);
            }
            other => panic!("expected a list value, got {other:?}"),
        }
    }

    #[test]
    fn notifying_dispatch_add_all_single() {
        let (n, ev) = recording_env();
        let mut list: NotifyingList<i32> = NotifyingList::with_notifier(n);
        list.add_all(vec![42]);
        let rec = ev.borrow();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].event(), EventType::Add);
        assert_eq!(rec[0].new_value, Val::Int(42));
    }
}
