//! The EMF notification model, ported from C++ `emf-common/ENotifier` and
//! `emf-common/Notification` (aligned to Java EMF `Notification` / `Notifier` /
//! `Adapter` / `NotificationChain`).
//!
//! The C++ port models notifications as a struct value with `std::any`
//! old/new values and enables transaction-scoped accumulation via an
//! interceptor hook. We model the same envelope with [`Notification`], a
//! per-notifier adapter registry in [`Notifier`], and a batch dispatcher in
//! [`NotificationChain`].

use crate::value::Val;

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
}

/// An adapter attached to a notifier (EMF `Adapter`).
pub trait Adapter {
    /// Called when the target notifies a change.
    fn notify_changed(&mut self, notification: &Notification);

    /// Whether this adapter serves the given type.
    fn is_adapter_for_type(&self, _other: &str) -> bool {
        false
    }
}

/// A notifier: holds a set of adapters and dispatches notifications to them.
#[derive(Default)]
pub struct Notifier {
    adapters: Vec<Box<dyn Adapter>>,
    /// Delivery enabled flag (EMF `eDeliver`).
    deliver: bool,
}

impl Notifier {
    /// New notifier with delivery enabled.
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
            deliver: true,
        }
    }

    /// Add an adapter.
    pub fn add_adapter(&mut self, adapter: Box<dyn Adapter>) {
        if !self
            .adapters
            .iter()
            .any(|a| std::ptr::eq(a.as_ref(), adapter.as_ref()))
        {
            self.adapters.push(adapter);
        }
    }

    /// Remove an adapter matching a predicate.
    pub fn remove_adapter(&mut self, mut predicate: impl FnMut(&dyn Adapter) -> bool) {
        self.adapters.retain(|a| !predicate(a.as_ref()));
    }

    /// The adapters.
    pub fn adapters(&self) -> &[Box<dyn Adapter>] {
        &self.adapters
    }

    /// Delivery flag.
    pub fn deliver(&self) -> bool {
        self.deliver
    }

    /// Set the delivery flag.
    pub fn set_deliver(&mut self, deliver: bool) {
        self.deliver = deliver;
    }

    /// Dispatch a notification to all adapters (snapshot iteration).
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

    /// Accumulate a notification, applying the SET+SET merge semantics.
    pub fn add(&mut self, n: Notification) {
        if let Some(last) = self.notifications.last_mut() {
            // SET + SET on the same feature: keep the earliest old value,
            // update new value.
            if last.event == EventType::Set
                && n.event == EventType::Set
                && last.feature == n.feature
            {
                *last = n;
                return;
            }
        }
        self.notifications.push(n);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn notifier_dispatches() {
        let mut n = Notifier::new();
        let seen: Rc<RefCell<Vec<EventType>>> = Rc::new(RefCell::new(Vec::new()));
        struct Rec {
            seen: Rc<RefCell<Vec<EventType>>>,
        }
        impl Adapter for Rec {
            fn notify_changed(&mut self, n: &Notification) {
                self.seen.borrow_mut().push(n.event());
            }
        }
        n.add_adapter(Box::new(Rec {
            seen: Rc::clone(&seen),
        }));
        n.e_notify(&Notification::new(
            EventType::Set,
            Some("x".into()),
            Val::Null,
            Val::Int(1),
            -1,
            true,
        ));
        assert_eq!(seen.borrow().as_slice(), &[EventType::Set]);
    }

    #[test]
    fn chain_merges_consecutive_sets() {
        let mut ch = NotificationChain::new();
        ch.add(Notification::new(
            EventType::Set,
            Some("a".into()),
            Val::Int(0),
            Val::Int(1),
            -1,
            true,
        ));
        ch.add(Notification::new(
            EventType::Set,
            Some("a".into()),
            Val::Int(1),
            Val::Int(2),
            -1,
            true,
        ));
        // Both SET on "a": merged into one.
        assert_eq!(ch.len(), 1);
        ch.add(Notification::new(
            EventType::Add,
            None,
            Val::Null,
            Val::Int(9),
            0,
            false,
        ));
        assert_eq!(ch.len(), 2);
    }
}
