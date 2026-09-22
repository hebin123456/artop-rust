//! The EMF notification model, ported from C++ `emf-common/ENotifier` and
//! `emf-common/Notification` (aligned to Java EMF `Notification` / `Notifier` /
//! `Adapter` / `NotificationChain`).
//!
//! The C++ port models notifications as a struct value with `std::any`
//! old/new values and enables transaction-scoped accumulation via an
//! interceptor hook. We model the same envelope with [`Notification`], a
//! per-notifier adapter registry in [`Notifier`], and a batch dispatcher in
//! [`NotificationChain`]. Behavior is kept equivalent to the C++ unit tests
//! (see `tools/conformance/cases.tsv`, group `enotifier`).

use std::rc::Rc;

use crate::value::{ObjectRef, Val};

/// Notification event kind (EMF `Notification.EventType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    /// An object was created.
    Create,
    /// A feature was set.
    Set,
    /// A feature was unset.
    Unset,
    /// An element was added.
    Add,
    /// An element was removed.
    Remove,
    /// Many elements were added.
    AddMany,
    /// Many elements were removed.
    RemoveMany,
    /// An element was moved.
    Move,
    /// An adapter is being removed.
    RemovingAdapter,
    /// A proxy was resolved.
    Resolve,
    /// Content type changed.
    ContentType,
}

impl EventType {
    /// Human name.
    pub fn name(&self) -> &'static str {
        match self {
            EventType::Create => "CREATE",
            EventType::Set => "SET",
            EventType::Unset => "UNSET",
            EventType::Add => "ADD",
            EventType::AddMany => "ADD_MANY",
            EventType::Remove => "REMOVE",
            EventType::RemoveMany => "REMOVE_MANY",
            EventType::Move => "MOVE",
            EventType::RemovingAdapter => "REMOVING_ADAPTER",
            EventType::Resolve => "RESOLVE",
            EventType::ContentType => "CONTENT_TYPE",
        }
    }
}

/// A notification event (EMF `Notification`).
#[derive(Debug, Clone)]
pub struct Notification {
    event: EventType,
    /// The structural feature name involved (if any).
    feature: Option<String>,
    /// Old value.
    pub old_value: Val,
    /// New value.
    pub new_value: Val,
    /// Position in a list, or -1.
    pub position: i32,
    /// Whether the isSet state changed.
    was_set: bool,
}

impl Notification {
    /// New notification.
    pub fn new(
        event: EventType,
        feature: Option<String>,
        old_value: Val,
        new_value: Val,
        position: i32,
        was_set: bool,
    ) -> Self {
        Self {
            event,
            feature,
            old_value,
            new_value,
            position,
            was_set,
        }
    }

    /// Event type.
    pub fn event(&self) -> EventType {
        self.event
    }

    /// Involved feature name.
    pub fn feature(&self) -> Option<&str> {
        self.feature.as_deref()
    }

    /// Whether the isSet state changed (EMF `Notification.wasSet`).
    pub fn was_set(&self) -> bool {
        self.was_set
    }

    /// Set the was-set flag.
    pub fn set_was_set(&mut self, was_set: bool) {
        self.was_set = was_set;
    }
}

/// An adapter attached to a notifier (EMF `Adapter`).
pub trait Adapter: std::any::Any {
    /// Called when the target notifies a change.
    fn notify_changed(&mut self, notification: &Notification);

    /// Whether this adapter serves the given type.
    fn is_adapter_for_type(&self, _other: &str) -> bool {
        false
    }

    /// The notifier this adapter is attached to, if any (EMF `Adapter.getTarget`).
    /// The value is an opaque notifier identity (its address); the default is
    /// `None` until [`Adapter::set_target`] is called.
    fn target(&self) -> Option<usize> {
        None
    }

    /// Record the notifier this adapter is attached to (`None` on removal).
    fn set_target(&mut self, _target: Option<usize>) {}

    /// Downcast handle to the concrete adapter (aligned to `EObject::as_any`).
    fn as_any(&self) -> &dyn std::any::Any;
}

/// A notifier: holds a set of adapters and dispatches notifications to them.
#[derive(Default)]
pub struct Notifier {
    adapters: Vec<Box<dyn Adapter>>,
    /// Delivery enabled flag (EMF `eDeliver`), defaults to `true`.
    deliver: bool,
}

impl Notifier {
    /// New notifier with delivery enabled (`eDeliver == true`).
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
            deliver: true,
        }
    }

    /// Add an adapter (duplicates are ignored, aligned to `eBasicAddAdapter`).
    /// The adapter's `target` is set to this notifier (EMF `addAdapter`).
    pub fn add_adapter(&mut self, adapter: Box<dyn Adapter>) {
        if !self
            .adapters
            .iter()
            .any(|a| std::ptr::eq(a.as_ref(), adapter.as_ref()))
        {
            let addr = self as *const Notifier as usize;
            self.adapters.push(adapter);
            self.adapters.last_mut().unwrap().set_target(Some(addr));
        }
    }

    /// Remove adapter(s) matching a predicate. When delivery is enabled, all
    /// *remaining* adapters are first sent a `REMOVING_ADAPTER` notification
    /// (aligned to C++ `removeAdapter`), then the matched adapters are dropped.
    pub fn remove_adapter(&mut self, mut predicate: impl FnMut(&dyn Adapter) -> bool) {
        let removing: Vec<*const dyn Adapter> = self
            .adapters
            .iter()
            .filter(|a| predicate(a.as_ref()))
            .map(|a| a.as_ref() as *const dyn Adapter)
            .collect();
        if removing.is_empty() {
            return;
        }
        if self.deliver {
            let evt = Notification::new(
                EventType::RemovingAdapter,
                None,
                Val::Null,
                Val::Null,
                -1,
                false,
            );
            for a in &mut self.adapters {
                let ptr = a.as_ref() as *const dyn Adapter;
                if !removing.contains(&ptr) {
                    a.notify_changed(&evt);
                }
            }
        }
        // Clear each removed adapter's target before dropping it
        // (EMF `removeAdapter` -> `eBasicRemoveAdapter`).
        for ptr in &removing {
            if let Some(a) = self
                .adapters
                .iter_mut()
                .find(|a| (a.as_ref() as *const dyn Adapter) == *ptr)
            {
                a.set_target(None);
            }
        }
        self.adapters
            .retain(|a| !removing.contains(&(a.as_ref() as *const dyn Adapter)));
    }

    /// The adapters (EMF `eAdapters`).
    pub fn adapters(&self) -> &[Box<dyn Adapter>] {
        &self.adapters
    }

    /// Alias of [`Self::adapters`] matching the C++ `eAdapters()` name.
    pub fn e_adapters(&self) -> &[Box<dyn Adapter>] {
        &self.adapters
    }

    /// Delivery flag (EMF `eDeliver`).
    pub fn deliver(&self) -> bool {
        self.deliver
    }

    /// Alias of [`Self::deliver`].
    pub fn e_deliver(&self) -> bool {
        self.deliver
    }

    /// Set the delivery flag (EMF `eSetDeliver`).
    pub fn set_deliver(&mut self, deliver: bool) {
        self.deliver = deliver;
    }

    /// Alias of [`Self::set_deliver`].
    pub fn e_set_deliver(&mut self, deliver: bool) {
        self.deliver = deliver;
    }

    /// Whether notifications are required: `eDeliver && !eAdapters().empty()`
    /// (EMF `eNotificationRequired`).
    pub fn e_notification_required(&self) -> bool {
        self.deliver && !self.adapters.is_empty()
    }

    /// Dispatch a notification to all adapters (snapshot iteration in order).
    pub fn e_notify(&mut self, n: &Notification) {
        if !self.deliver {
            return;
        }
        for a in &mut self.adapters {
            a.notify_changed(n);
        }
    }
}

/// A batch of notifications dispatched together (EMF `NotificationChain`).
#[derive(Debug, Clone, Default)]
pub struct NotificationChain {
    notifications: Vec<Notification>,
}

impl NotificationChain {
    /// New empty chain.
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
        }
    }

    /// The queued notifications.
    pub fn notifications(&self) -> &[Notification] {
        &self.notifications
    }

    /// Accumulate a notification, applying EMF merge/cancel semantics:
    ///
    /// - `SET` + `SET` on the same feature/notifier merges into one: the
    ///   earliest old value is kept, the latest new value wins.
    /// - `ADD` + `REMOVE` of the same object/feature/position cancel to a
    ///   no-op.
    pub fn add(&mut self, n: Notification) {
        if let Some(last) = self.notifications.last_mut() {
            // SET + SET on the same feature: keep earliest old value, take
            // latest new value (aligned to C++ NotificationChain).
            if last.event == EventType::Set
                && n.event == EventType::Set
                && last.feature == n.feature
            {
                last.new_value = n.new_value;
                last.position = n.position;
                last.was_set = n.was_set;
                return;
            }
            // ADD + REMOVE of the same object/feature/position cancel both.
            if last.event == EventType::Add
                && n.event == EventType::Remove
                && last.feature == n.feature
                && last.position == n.position
                && obj_key(&last.new_value).is_some()
                && obj_key(&last.new_value) == obj_key(&n.old_value)
            {
                self.notifications.pop();
                return;
            }
        }
        self.notifications.push(n);
    }

    /// Merge another collection of notifications into this chain, draining it
    /// (aligned to C++ `NotificationChain::merge(std::move(other))`). Each item
    /// is added through [`Self::add`], so merge/cancel semantics apply.
    pub fn merge(&mut self, other: &mut Vec<Notification>) {
        for n in other.drain(..) {
            self.add(n);
        }
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
    }

    /// Number of queued notifications.
    pub fn len(&self) -> usize {
        self.notifications.len()
    }

    /// Dispatch all queued notifications to a notifier.
    pub fn dispatch(&mut self, notifier: &mut Notifier) {
        let drained = std::mem::take(&mut self.notifications);
        for n in drained {
            notifier.e_notify(&n);
        }
    }
}

/// Object-identity key for a `Val` that holds an `ObjectRef`.
fn obj_key(v: &Val) -> Option<usize> {
    v.as_object().map(|o| Rc::as_ptr(o) as *const () as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A recording adapter that logs `(id, event)` into a shared buffer and
    /// carries a small fake EObject for identity comparisons.
    struct Rec {
        id: u32,
        out: Rc<RefCell<Vec<(u32, EventType)>>>,
    }
    impl Rec {
        fn new(id: u32, out: Rc<RefCell<Vec<(u32, EventType)>>>) -> Box<dyn Adapter> {
            Box::new(Self { id, out })
        }
        fn count(&self, out: &Rc<RefCell<Vec<(u32, EventType)>>>) -> usize {
            out.borrow().iter().filter(|(i, _)| *i == self.id).count()
        }
    }
    impl Adapter for Rec {
        fn notify_changed(&mut self, notification: &Notification) {
            self.out.borrow_mut().push((self.id, notification.event()));
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    fn rec_with_id(out: &Rc<RefCell<Vec<(u32, EventType)>>>, id: u32) -> Box<dyn Adapter> {
        Rec::new(id, Rc::clone(out))
    }

    fn set_evt(feature: Option<&str>, old: &str, new: &str) -> Notification {
        Notification::new(
            EventType::Set,
            feature.map(|s| s.to_string()),
            Val::String(old.into()),
            Val::String(new.into()),
            -1,
            false,
        )
    }

    #[test]
    fn notifier_default_e_deliver_true() {
        let n = Notifier::new();
        assert!(n.e_deliver());
    }

    #[test]
    fn notifier_default_empty_adapters() {
        let n = Notifier::new();
        assert!(n.e_adapters().is_empty());
    }

    #[test]
    fn notifier_add_adapter_increases_size() {
        let out: Rc<RefCell<Vec<(u32, EventType)>>> = Rc::new(RefCell::new(vec![]));
        let mut n = Notifier::new();
        n.add_adapter(rec_with_id(&out, 1));
        assert_eq!(n.adapters().len(), 1);
    }

    #[test]
    fn notifier_add_multiple_adapters() {
        let out: Rc<RefCell<Vec<(u32, EventType)>>> = Rc::new(RefCell::new(vec![]));
        let mut n = Notifier::new();
        n.add_adapter(rec_with_id(&out, 1));
        n.add_adapter(rec_with_id(&out, 2));
        n.add_adapter(rec_with_id(&out, 3));
        assert_eq!(n.adapters().len(), 3);
    }

    #[test]
    fn notifier_remove_adapter_decreases_size() {
        let out: Rc<RefCell<Vec<(u32, EventType)>>> = Rc::new(RefCell::new(vec![]));
        let mut n = Notifier::new();
        n.add_adapter(rec_with_id(&out, 1));
        assert_eq!(n.adapters().len(), 1);
        n.remove_adapter(|a| a.as_any().downcast_ref::<Rec>().is_some_and(|r| r.id == 1));
        assert_eq!(n.adapters().len(), 0);
    }

    #[test]
    fn notifier_remove_adapter_not_present_no_change() {
        let out: Rc<RefCell<Vec<(u32, EventType)>>> = Rc::new(RefCell::new(vec![]));
        let mut n = Notifier::new();
        n.add_adapter(rec_with_id(&out, 1));
        n.remove_adapter(|a| a.as_any().downcast_ref::<Rec>().is_some_and(|r| r.id == 99));
        assert_eq!(n.adapters().len(), 1);
    }

    #[test]
    fn notifier_remove_adapter_middle_preserves_order() {
        let out: Rc<RefCell<Vec<(u32, EventType)>>> = Rc::new(RefCell::new(vec![]));
        let mut n = Notifier::new();
        n.add_adapter(rec_with_id(&out, 1));
        n.add_adapter(rec_with_id(&out, 2));
        n.add_adapter(rec_with_id(&out, 3));
        n.remove_adapter(|a| a.as_any().downcast_ref::<Rec>().is_some_and(|r| r.id == 2));
        assert_eq!(n.adapters().len(), 2);
        // order preserved: 1 then 3
        assert!(n
            .adapters()
            .first()
            .unwrap()
            .as_any()
            .downcast_ref::<Rec>()
            .is_some_and(|r| r.id == 1));
        assert!(n
            .adapters()
            .last()
            .unwrap()
            .as_any()
            .downcast_ref::<Rec>()
            .is_some_and(|r| r.id == 3));
    }

    #[test]
    fn notifier_eset_deliver_toggle() {
        let mut n = Notifier::new();
        assert!(n.e_deliver());
        n.e_set_deliver(false);
        assert!(!n.e_deliver());
        n.e_set_deliver(true);
        assert!(n.e_deliver());
    }

    #[test]
    fn notifier_enotify_e_deliver_false_no_delivery() {
        let out: Rc<RefCell<Vec<(u32, EventType)>>> = Rc::new(RefCell::new(vec![]));
        let mut n = Notifier::new();
        n.add_adapter(rec_with_id(&out, 1));
        n.e_set_deliver(false);
        n.e_notify(&set_evt(None, "old", "new"));
        assert!(out.borrow().is_empty());
    }

    #[test]
    fn notifier_enotify_e_deliver_true_all() {
        let out: Rc<RefCell<Vec<(u32, EventType)>>> = Rc::new(RefCell::new(vec![]));
        let mut n = Notifier::new();
        n.add_adapter(rec_with_id(&out, 1));
        n.add_adapter(rec_with_id(&out, 2));
        n.e_set_deliver(true);
        n.e_notify(&set_evt(None, "old", "new"));
        let rec = out.borrow();
        assert_eq!(rec.len(), 2);
        assert_eq!(rec[0], (1, EventType::Set));
        assert_eq!(rec[1], (2, EventType::Set));
    }

    #[test]
    fn notifier_enotify_no_adapters_no_crash() {
        let mut n = Notifier::new();
        n.e_set_deliver(true);
        n.e_notify(&set_evt(None, "old", "new"));
    }

    #[test]
    fn notifier_enotification_required_no_adapters_false() {
        let n = Notifier::new();
        assert!(!n.e_notification_required());
    }

    #[test]
    fn notifier_enotification_required_deliver_false_false() {
        let out: Rc<RefCell<Vec<(u32, EventType)>>> = Rc::new(RefCell::new(vec![]));
        let mut n = Notifier::new();
        n.add_adapter(rec_with_id(&out, 1));
        n.e_set_deliver(false);
        assert!(!n.e_notification_required());
    }

    #[test]
    fn notifier_enotification_required_deliver_true_true() {
        let out: Rc<RefCell<Vec<(u32, EventType)>>> = Rc::new(RefCell::new(vec![]));
        let mut n = Notifier::new();
        n.add_adapter(rec_with_id(&out, 1));
        n.e_set_deliver(true);
        assert!(n.e_notification_required());
    }

    #[test]
    fn notifier_remove_adapter_notifies_remaining() {
        let out: Rc<RefCell<Vec<(u32, EventType)>>> = Rc::new(RefCell::new(vec![]));
        let mut n = Notifier::new();
        n.e_set_deliver(true);
        n.add_adapter(rec_with_id(&out, 1));
        n.add_adapter(rec_with_id(&out, 2));
        n.remove_adapter(|a| a.as_any().downcast_ref::<Rec>().is_some_and(|r| r.id == 1));
        // a2 (remaining) got exactly one REMOVING_ADAPTER.
        let rec = out.borrow();
        let ids: Vec<(u32, EventType)> = rec.iter().filter(|(i, _)| *i == 2).cloned().collect();
        assert_eq!(ids, vec![(2, EventType::RemovingAdapter)]);
    }

    #[test]
    fn notifier_remove_adapter_e_deliver_false_no_notification() {
        let out: Rc<RefCell<Vec<(u32, EventType)>>> = Rc::new(RefCell::new(vec![]));
        let mut n = Notifier::new();
        n.e_set_deliver(false);
        n.add_adapter(rec_with_id(&out, 1));
        n.add_adapter(rec_with_id(&out, 2));
        n.remove_adapter(|a| a.as_any().downcast_ref::<Rec>().is_some_and(|r| r.id == 1));
        assert!(out.borrow().is_empty());
        assert_eq!(n.adapters().len(), 1);
    }

    #[test]
    fn notification_was_set_default_false() {
        let n = Notification::new(EventType::Set, None, Val::Null, Val::Int(1), -1, false);
        assert!(!n.was_set());
    }

    #[test]
    fn notification_was_set_constructor_and_setter() {
        let mut n = Notification::new(EventType::Set, None, Val::Null, Val::Int(1), -1, true);
        assert!(n.was_set());
        n.set_was_set(false);
        assert!(!n.was_set());
    }

    #[test]
    fn chain_set_set_merges_keeps_earliest_old_and_latest_new() {
        let mut ch = NotificationChain::new();
        ch.add(set_evt(None, "first", "a"));
        ch.add(set_evt(None, "b", "second"));
        assert_eq!(ch.len(), 1);
        let n = &ch.notifications()[0];
        assert_eq!(n.old_value, Val::String("first".into()));
        assert_eq!(n.new_value, Val::String("second".into()));
    }

    fn add_evt(obj: &ObjectRef) -> Notification {
        Notification::new(
            EventType::Add,
            None,
            Val::Null,
            Val::Object(Rc::clone(obj)),
            0,
            false,
        )
    }
    fn remove_evt(obj: &ObjectRef) -> Notification {
        Notification::new(
            EventType::Remove,
            None,
            Val::Object(Rc::clone(obj)),
            Val::Null,
            0,
            false,
        )
    }

    #[test]
    fn chain_add_remove_cancels() {
        let obj: ObjectRef = Rc::new(RefCell::new(TestEObject));
        let mut ch = NotificationChain::new();
        ch.add(add_evt(&obj));
        assert_eq!(ch.len(), 1);
        ch.add(remove_evt(&obj));
        assert_eq!(ch.len(), 0); // cancelled
    }

    #[test]
    fn chain_add_remove_different_object_not_cancelled() {
        let o1: ObjectRef = Rc::new(RefCell::new(TestEObject));
        let o2: ObjectRef = Rc::new(RefCell::new(TestEObject));
        let mut ch = NotificationChain::new();
        ch.add(add_evt(&o1));
        ch.add(remove_evt(&o2));
        assert_eq!(ch.len(), 2);
    }

    #[test]
    fn chain_merge_drains_source_and_merges() {
        let mut ch = NotificationChain::new();
        ch.add(set_evt(None, "first", "a"));
        let mut other = vec![set_evt(None, "b", "second")];
        ch.merge(&mut other);
        assert!(other.is_empty()); // source drained
        assert_eq!(ch.len(), 1); // SET+SET merged
    }

    #[test]
    fn chain_merge_append_all_for_different_features() {
        let mut ch = NotificationChain::new();
        ch.add(set_evt(Some("a"), "0", "1"));
        let mut other = vec![set_evt(Some("b"), "0", "1")];
        ch.merge(&mut other);
        assert_eq!(ch.len(), 2); // different features, no merge
    }

    #[test]
    fn chain_merge_add_remove_cancels() {
        let obj: ObjectRef = Rc::new(RefCell::new(TestEObject));
        let mut ch = NotificationChain::new();
        ch.add(add_evt(&obj));
        let mut other = vec![remove_evt(&obj)];
        ch.merge(&mut other);
        assert!(other.is_empty()); // source drained
        assert_eq!(ch.len(), 0); // ADD+REMOVE cancelled via merge
    }

    #[test]
    fn chain_merge_then_dispatch_delivers_merged() {
        let out: Rc<RefCell<Vec<(u32, EventType)>>> = Rc::new(RefCell::new(vec![]));
        let mut n = Notifier::new();
        n.add_adapter(rec_with_id(&out, 1));
        n.e_set_deliver(true);

        let mut ch = NotificationChain::new();
        ch.add(set_evt(None, "old1", "v1"));
        let mut other = vec![set_evt(None, "v1", "v2")];
        ch.merge(&mut other);
        assert_eq!(ch.len(), 1);

        ch.dispatch(&mut n);
        // adapter got exactly one notification, kept earliest old/value latest.
        let rec = out.borrow();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].1, EventType::Set);
    }

    #[derive(Debug)]
    struct TestEObject;
    impl crate::eobject::EObject for TestEObject {
        fn e_class(&self) -> &str {
            "TestEObject"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    // ---- EAdapter target management + field-preserving delivery ----

    /// Adapter whose `target` value lives in a shared cell so it stays
    /// observable after the adapter is dropped on removal.
    struct TargetedRec {
        seal: Rc<RefCell<Option<usize>>>,
        events: Rc<RefCell<Vec<Notification>>>,
    }
    impl Adapter for TargetedRec {
        fn notify_changed(&mut self, n: &Notification) {
            self.events.borrow_mut().push(n.clone());
        }
        fn target(&self) -> Option<usize> {
            *self.seal.borrow()
        }
        fn set_target(&mut self, t: Option<usize>) {
            *self.seal.borrow_mut() = t;
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[test]
    fn adapter_default_target_null() {
        let a = TargetedRec {
            seal: Rc::new(RefCell::new(None)),
            events: Rc::new(RefCell::new(vec![])),
        };
        assert!(a.target().is_none());
    }

    #[test]
    fn adapter_set_target_returns_same() {
        let mut a = TargetedRec {
            seal: Rc::new(RefCell::new(None)),
            events: Rc::new(RefCell::new(vec![])),
        };
        a.set_target(Some(42));
        assert_eq!(a.target(), Some(42));
    }

    #[test]
    fn notifier_add_adapter_sets_target() {
        let seal: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(None));
        let mut n = Notifier::new();
        n.add_adapter(Box::new(TargetedRec {
            seal: Rc::clone(&seal),
            events: Rc::new(RefCell::new(vec![])),
        }));
        let addr = &n as *const Notifier as usize;
        assert_eq!(*seal.borrow(), Some(addr));
    }

    #[test]
    fn notifier_remove_adapter_clears_target() {
        let seal: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(None));
        let events: Rc<RefCell<Vec<Notification>>> = Rc::new(RefCell::new(vec![]));
        let mut n = Notifier::new();
        n.add_adapter(Box::new(TargetedRec {
            seal: Rc::clone(&seal),
            events: Rc::clone(&events),
        }));
        assert!(seal.borrow().is_some());
        n.remove_adapter(|a| a.as_any().downcast_ref::<TargetedRec>().is_some());
        assert_eq!(*seal.borrow(), None);
    }

    #[test]
    fn notifier_enotify_preserves_event_fields() {
        let events: Rc<RefCell<Vec<Notification>>> = Rc::new(RefCell::new(vec![]));
        let mut n = Notifier::new();
        n.add_adapter(Box::new(TargetedRec {
            seal: Rc::new(RefCell::new(None)),
            events: Rc::clone(&events),
        }));
        n.e_set_deliver(true);
        let evt = Notification::new(
            EventType::Add,
            None,
            Val::String("x".into()),
            Val::String("y".into()),
            3,
            false,
        );
        n.e_notify(&evt);
        let rec = events.borrow();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].event(), EventType::Add);
        assert_eq!(rec[0].position, 3);
        assert!(rec[0].feature().is_none());
        assert_eq!(rec[0].old_value, Val::String("x".into()));
        assert_eq!(rec[0].new_value, Val::String("y".into()));
    }

    // ---- EObjectImpl-like reverse containment notifications ----

    /// Minimal object holding a `Notifier` plus a containment slot, so
    /// `set_e_container` can fire reverse ADD/REMOVE notifications (feature
    /// is `None`, aligning to the reverse path in C++ EObjectImpl).
    struct ContainerObj {
        notifier: Notifier,
        container: RefCell<Option<ObjectRef>>,
    }
    impl std::fmt::Debug for ContainerObj {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("ContainerObj")
        }
    }
    impl Default for ContainerObj {
        fn default() -> Self {
            Self {
                notifier: Notifier::new(),
                container: RefCell::new(None),
            }
        }
    }
    impl crate::eobject::EObject for ContainerObj {
        fn e_class(&self) -> &str {
            "ContainerObj"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }
    impl ContainerObj {
        fn add_adapter(&mut self, a: Box<dyn Adapter>) {
            self.notifier.add_adapter(a);
        }
        fn set_e_container(&mut self, new: ObjectRef) {
            let same = matches!(
                self.container.borrow().as_ref(),
                Some(old) if Rc::ptr_eq(old, &new)
            );
            if same {
                return;
            }
            let old = self.container.borrow_mut().replace(new.clone());
            if let Some(o) = old {
                let r = Notification::new(
                    EventType::Remove,
                    None,
                    Val::Object(o),
                    Val::Null,
                    -1,
                    false,
                );
                self.notifier.e_notify(&r);
            }
            let a = Notification::new(EventType::Add, None, Val::Null, Val::Object(new), -1, false);
            self.notifier.e_notify(&a);
        }
    }

    /// Concrete handle to a `ContainerObj` (so `set_e_container` is reachable).
    type ContainerHandle = Rc<RefCell<ContainerObj>>;
    fn container_obj() -> ContainerHandle {
        Rc::new(RefCell::new(ContainerObj::default()))
    }
    fn targeted_child(events: &Rc<RefCell<Vec<Notification>>>) -> ContainerHandle {
        let child = container_obj();
        child.borrow_mut().add_adapter(Box::new(TargetedRec {
            seal: Rc::new(RefCell::new(None)),
            events: Rc::clone(events),
        }));
        child
    }

    #[test]
    fn eobject_set_e_container_fires_reverse_add() {
        let events: Rc<RefCell<Vec<Notification>>> = Rc::new(RefCell::new(vec![]));
        let child = targeted_child(&events);
        let parent: ObjectRef = Rc::new(RefCell::new(ContainerObj::default()));
        child.borrow_mut().set_e_container(parent.clone());
        let rec = events.borrow();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].event(), EventType::Add);
        assert!(rec[0].feature().is_none()); // reverse: no feature
        assert!(matches!(
            &rec[0].new_value,
            Val::Object(o) if Rc::ptr_eq(o, &parent)
        ));
    }

    #[test]
    fn eobject_set_e_container_switch_fires_remove_then_add() {
        let events: Rc<RefCell<Vec<Notification>>> = Rc::new(RefCell::new(vec![]));
        let child = targeted_child(&events);
        let p1: ObjectRef = Rc::new(RefCell::new(ContainerObj::default()));
        let p2: ObjectRef = Rc::new(RefCell::new(ContainerObj::default()));
        child.borrow_mut().set_e_container(p1.clone()); // ADD(p1)
        child.borrow_mut().set_e_container(p2.clone()); // REMOVE(p1)+ADD(p2)
        let rec = events.borrow();
        assert_eq!(rec.len(), 3);
        assert_eq!(rec[1].event(), EventType::Remove);
        assert!(matches!(&rec[1].old_value, Val::Object(o) if Rc::ptr_eq(o, &p1)));
        assert_eq!(rec[2].event(), EventType::Add);
        assert!(matches!(&rec[2].new_value, Val::Object(o) if Rc::ptr_eq(o, &p2)));
    }

    #[test]
    fn eobject_set_e_container_same_container_no_notification() {
        let events: Rc<RefCell<Vec<Notification>>> = Rc::new(RefCell::new(vec![]));
        let child = targeted_child(&events);
        let parent: ObjectRef = Rc::new(RefCell::new(ContainerObj::default()));
        child.borrow_mut().set_e_container(parent.clone());
        child.borrow_mut().set_e_container(parent.clone()); // no change
        assert_eq!(events.borrow().len(), 1);
    }
}
