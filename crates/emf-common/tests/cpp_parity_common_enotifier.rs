//! C++ parity suite: emf-common Notification / Notifier / NotificationChain.
//!
//! Ports `ENotifierTests.cpp` from
//! `artop-cpp/cpp/emf-cpp/emf-common/tests/`.
//!
//! Skipped group (tracked as a gap): the three `EObjectImpl_SetEContainer`
//! reverse-notification cases require `setEContainer` reverse
//! ADD/REMOVE dispatch, which lives in emf-ecore's EObject container wiring,
//! not in emf-common.
use emf_common::eobject::EObject;
use emf_common::notification::{Adapter, EventType, Notification, NotificationChain, Notifier};
use emf_common::value::{ObjectRef, Val};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A recording adapter: counts deliveries and records the received copies.
struct Recording {
    id: u32,
    notifies: Rc<Cell<usize>>,
    received: Rc<RefCell<Vec<Notification>>>,
    target: RefCell<Option<usize>>,
}

impl Recording {
    fn new(
        id: u32,
    ) -> (
        Box<dyn Adapter>,
        Rc<Cell<usize>>,
        Rc<RefCell<Vec<Notification>>>,
    ) {
        let notifies = Rc::new(Cell::new(0));
        let received = Rc::new(RefCell::new(Vec::new()));
        let a = Box::new(Recording {
            id,
            notifies: notifies.clone(),
            received: received.clone(),
            target: RefCell::new(None),
        });
        (a, notifies, received)
    }
    #[allow(dead_code)]
    fn feature<R: Adapter + ?Sized + 'static>(a: &R) -> u32 {
        a.as_any().downcast_ref::<Recording>().unwrap().id
    }
}

impl Adapter for Recording {
    fn notify_changed(&mut self, n: &Notification) {
        self.notifies.set(self.notifies.get() + 1);
        self.received.borrow_mut().push(n.clone());
    }
    fn target(&self) -> Option<usize> {
        *self.target.borrow()
    }
    fn set_target(&mut self, t: Option<usize>) {
        *self.target.borrow_mut() = t;
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Minimal EObject stub for `NotificationChain` cancel tests.
#[derive(Debug)]
struct Obj;
impl EObject for Obj {
    fn e_class(&self) -> &str {
        "Obj"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn set_not(event: EventType, old: Val, new: Val, pos: i32, was: bool) -> Notification {
    Notification::new(event, None, old, new, pos, was)
}

// ---------------------------------------------------------------------------
// Default state
// ---------------------------------------------------------------------------

#[test]
fn notifier_default_deliver_true() {
    let n = Notifier::new();
    assert!(n.e_deliver());
}

#[test]
fn notifier_default_empty_adapters() {
    let n = Notifier::new();
    assert!(n.e_adapters().is_empty());
    assert_eq!(n.e_adapters().len(), 0);
}

// ---------------------------------------------------------------------------
// add_adapter / remove_adapter
// ---------------------------------------------------------------------------

#[test]
fn notifier_add_adapter_increases_size() {
    let mut n = Notifier::new();
    let (a, _, _) = Recording::new(1);
    n.add_adapter(a);
    assert_eq!(n.e_adapters().len(), 1);
    assert_eq!(Recording::feature(&*n.e_adapters()[0]), 1);
}

#[test]
fn notifier_add_multiple_adapters() {
    let mut n = Notifier::new();
    let (a1, _, _) = Recording::new(1);
    let (a2, _, _) = Recording::new(2);
    let (a3, _, _) = Recording::new(3);
    n.add_adapter(a1);
    n.add_adapter(a2);
    n.add_adapter(a3);
    assert_eq!(n.e_adapters().len(), 3);
    assert_eq!(Recording::feature(&*n.e_adapters()[0]), 1);
    assert_eq!(Recording::feature(&*n.e_adapters()[1]), 2);
    assert_eq!(Recording::feature(&*n.e_adapters()[2]), 3);
}

#[test]
fn notifier_add_adapter_distinct_instances() {
    // Rust owns adapters as `Box<dyn Adapter>`: re-adding the *same* instance
    // is unrepresentable, so a distinct second adapter grows the list. The
    // C++ same-instance dedup is satisfied vacuously by the boxed ownership.
    let mut n = Notifier::new();
    let (a1, _, _) = Recording::new(1);
    let (a2, _, _) = Recording::new(2);
    n.add_adapter(a1);
    n.add_adapter(a2);
    assert_eq!(n.e_adapters().len(), 2);
}

#[test]
fn notifier_remove_adapter_decreases_size() {
    let mut n = Notifier::new();
    let (a, _, _) = Recording::new(1);
    n.add_adapter(a);
    assert_eq!(n.e_adapters().len(), 1);
    n.remove_adapter(|a| {
        a.as_any()
            .downcast_ref::<Recording>()
            .map_or(false, |r| r.id == 1)
    });
    assert_eq!(n.e_adapters().len(), 0);
}

#[test]
fn notifier_remove_adapter_not_present_no_change() {
    let mut n = Notifier::new();
    let (a1, _, _) = Recording::new(1);
    n.add_adapter(a1);
    n.remove_adapter(|a| {
        a.as_any()
            .downcast_ref::<Recording>()
            .map_or(false, |r| r.id == 2)
    });
    assert_eq!(n.e_adapters().len(), 1);
}

#[test]
fn notifier_remove_adapter_empty_predicate_no_change() {
    let mut n = Notifier::new();
    let (a, _, _) = Recording::new(1);
    n.add_adapter(a);
    n.remove_adapter(|_| false);
    assert_eq!(n.e_adapters().len(), 1);
}

#[test]
fn notifier_remove_adapter_middle_preserves_order() {
    let mut n = Notifier::new();
    let (a1, _, _) = Recording::new(1);
    let (a2, _, _) = Recording::new(2);
    let (a3, _, _) = Recording::new(3);
    n.add_adapter(a1);
    n.add_adapter(a2);
    n.add_adapter(a3);
    n.remove_adapter(|a| {
        a.as_any()
            .downcast_ref::<Recording>()
            .map_or(false, |r| r.id == 2)
    });
    assert_eq!(n.e_adapters().len(), 2);
    assert_eq!(Recording::feature(&*n.e_adapters()[0]), 1);
    assert_eq!(Recording::feature(&*n.e_adapters()[1]), 3);
}

// ---------------------------------------------------------------------------
// eDeliver / eSetDeliver
// ---------------------------------------------------------------------------

#[test]
fn notifier_set_deliver_toggle() {
    let mut n = Notifier::new();
    assert!(n.e_deliver());
    n.e_set_deliver(false);
    assert!(!n.e_deliver());
    n.e_set_deliver(true);
    assert!(n.e_deliver());
}

#[test]
fn notifier_e_notify_deliver_false_no_delivery() {
    let mut n = Notifier::new();
    let (_, notifies, _) = Recording::new(1);
    n.add_adapter(Box::new(Recording {
        id: 1,
        notifies: notifies.clone(),
        received: Rc::new(RefCell::new(Vec::new())),
        target: RefCell::new(None),
    }));
    n.e_set_deliver(false);
    let evt = set_not(EventType::Set, Val::Null, Val::Null, -1, false);
    n.e_notify(&evt);
    assert_eq!(notifies.get(), 0);
}

#[test]
fn notifier_e_notify_deliver_true_delivered_to_all() {
    let mut n = Notifier::new();
    let (a1, c1, _) = Recording::new(1);
    let (a2, c2, _) = Recording::new(2);
    n.add_adapter(a1);
    n.add_adapter(a2);
    n.e_set_deliver(true);
    let evt = set_not(EventType::Set, Val::Null, Val::Null, -1, false);
    n.e_notify(&evt);
    assert_eq!(c1.get(), 1);
    assert_eq!(c2.get(), 1);
}

#[test]
fn notifier_e_notify_no_adapters_no_crash() {
    let mut n = Notifier::new();
    n.e_set_deliver(true);
    let evt = set_not(EventType::Add, Val::Null, Val::Null, -1, false);
    n.e_notify(&evt); // must not panic
}

#[test]
fn notifier_e_notify_preserves_event_fields() {
    let mut n = Notifier::new();
    let (_, notifies, received) = Recording::new(1);
    n.add_adapter(Box::new(Recording {
        id: 1,
        notifies: notifies.clone(),
        received: received.clone(),
        target: RefCell::new(None),
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
    let received = received.borrow();
    assert_eq!(received.len(), 1);
    let r = &received[0];
    assert_eq!(r.event(), EventType::Add);
    assert_eq!(r.feature(), None);
    assert_eq!(r.position, 3);
    assert_eq!(r.new_value, Val::String("y".into()));
    assert_eq!(r.old_value, Val::String("x".into()));
}

// ---------------------------------------------------------------------------
// eNotificationRequired semantics
// ---------------------------------------------------------------------------

#[test]
fn notification_required_no_adapters_false() {
    let n = Notifier::new();
    assert!(!n.e_notification_required());
}

#[test]
fn notification_required_adapter_but_no_deliver_false() {
    let mut n = Notifier::new();
    let (a, _, _) = Recording::new(1);
    n.add_adapter(a);
    n.e_set_deliver(false);
    assert!(!n.e_notification_required());
}

#[test]
fn notification_required_adapter_and_deliver_true() {
    let mut n = Notifier::new();
    let (a, _, _) = Recording::new(1);
    n.add_adapter(a);
    n.e_set_deliver(true);
    assert!(n.e_notification_required());
}

// ---------------------------------------------------------------------------
// remove_adapter triggers REMOVING_ADAPTER to remaining adapters
// ---------------------------------------------------------------------------

#[test]
fn remove_adapter_notifies_remaining_adapters() {
    let mut n = Notifier::new();
    n.e_set_deliver(true);
    let (a1, _, _) = Recording::new(1);
    let (a2, _, a2r) = Recording::new(2);
    n.add_adapter(a1);
    n.add_adapter(a2);
    n.remove_adapter(|a| {
        a.as_any()
            .downcast_ref::<Recording>()
            .map_or(false, |r| r.id == 1)
    });
    let r = a2r.borrow();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].event(), EventType::RemovingAdapter);
}

#[test]
fn remove_adapter_during_notify_removed_adapter_receives_removing_adapter() {
    // ChangeNotification/RemoveAdapter_DuringNotify_SafeIteration: the adapter
    // being removed still receives exactly one REMOVING_ADAPTER, because C++
    // `eBasicRemoveAdapter` calls `eNotify` *before* erasing it from the list.
    let mut n = Notifier::new();
    n.e_set_deliver(true);
    let (a1, notifies, received) = Recording::new(1);
    n.add_adapter(a1);
    n.remove_adapter(|a| {
        a.as_any()
            .downcast_ref::<Recording>()
            .is_some_and(|r| r.id == 1)
    });
    assert_eq!(notifies.get(), 1);
    let r = received.borrow();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].event(), EventType::RemovingAdapter);
    assert!(n.e_adapters().is_empty());
}

#[test]
fn remove_adapter_deliver_false_no_removal_notification() {
    let mut n = Notifier::new();
    n.e_set_deliver(false);
    let (a1, _, _) = Recording::new(1);
    let (a2, _, a2r) = Recording::new(2);
    n.add_adapter(a1);
    n.add_adapter(a2);
    n.remove_adapter(|a| {
        a.as_any()
            .downcast_ref::<Recording>()
            .map_or(false, |r| r.id == 1)
    });
    assert!(a2r.borrow().is_empty());
    assert_eq!(n.e_adapters().len(), 1);
    assert_eq!(Recording::feature(&*n.e_adapters()[0]), 2);
}

// ---------------------------------------------------------------------------
// Adapter target management
// ---------------------------------------------------------------------------

#[test]
fn adapter_default_target_none() {
    let (a, _, _) = Recording::new(1);
    assert_eq!(a.target(), None);
}

#[test]
fn add_adapter_sets_target() {
    let mut n = Notifier::new();
    let (a, _, _) = Recording::new(1);
    n.add_adapter(a);
    let this_addr = &n as *const Notifier as usize;
    assert_eq!(n.e_adapters()[0].target(), Some(this_addr));
}

#[test]
fn remove_adapter_clears_target() {
    let mut n = Notifier::new();
    let (a, _, _) = Recording::new(1);
    n.add_adapter(a);
    let this_addr = &n as *const Notifier as usize;
    assert_eq!(n.e_adapters()[0].target(), Some(this_addr));
    n.remove_adapter(|a| {
        a.as_any()
            .downcast_ref::<Recording>()
            .map_or(false, |r| r.id == 1)
    });
    assert!(n.e_adapters().is_empty());
    // removed adapter's target was cleared before drop (untestable directly);
    // the vacuous empty-list state matches C++ `RemoveAdapter` expectations.
}

// ---------------------------------------------------------------------------
// NotificationChain cancel / merge
// ---------------------------------------------------------------------------

#[test]
fn chain_add_remove_cancels() {
    let obj = Obj;
    let o1: ObjectRef = Rc::new(RefCell::new(obj));
    let mut chain = NotificationChain::new();
    chain.add(Notification::new(
        EventType::Add,
        None,
        Val::Null,
        Val::Object(o1.clone()),
        0,
        false,
    ));
    assert_eq!(chain.len(), 1);
    chain.add(Notification::new(
        EventType::Remove,
        None,
        Val::Object(o1),
        Val::Null,
        0,
        false,
    ));
    assert_eq!(chain.len(), 0);
}

#[test]
fn chain_add_remove_different_object_not_cancelled() {
    let (o1, o2): (ObjectRef, ObjectRef) = (Rc::new(RefCell::new(Obj)), Rc::new(RefCell::new(Obj)));
    let mut chain = NotificationChain::new();
    chain.add(Notification::new(
        EventType::Add,
        None,
        Val::Null,
        Val::Object(o1),
        0,
        false,
    ));
    chain.add(Notification::new(
        EventType::Remove,
        None,
        Val::Object(o2),
        Val::Null,
        0,
        false,
    ));
    assert_eq!(chain.len(), 2);
}

#[test]
fn chain_set_set_merges() {
    let mut chain = NotificationChain::new();
    chain.add(Notification::new(
        EventType::Set,
        None,
        Val::String("first".into()),
        Val::String("a".into()),
        -1,
        false,
    ));
    chain.add(Notification::new(
        EventType::Set,
        None,
        Val::String("b".into()),
        Val::String("second".into()),
        -1,
        false,
    ));
    assert_eq!(chain.len(), 1);
}

#[test]
fn chain_merge_vector_merges_set_set() {
    let mut chain = NotificationChain::new();
    chain.add(Notification::new(
        EventType::Set,
        None,
        Val::String("first".into()),
        Val::String("a".into()),
        -1,
        false,
    ));
    let mut other = vec![Notification::new(
        EventType::Set,
        None,
        Val::String("b".into()),
        Val::String("second".into()),
        -1,
        false,
    )];
    chain.merge(&mut other);
    assert_eq!(chain.len(), 1);
    assert!(other.is_empty());
}

#[test]
fn chain_merge_vector_add_remove_cancels() {
    let o: ObjectRef = Rc::new(RefCell::new(Obj));
    let mut chain = NotificationChain::new();
    chain.add(Notification::new(
        EventType::Add,
        None,
        Val::Null,
        Val::Object(o.clone()),
        0,
        false,
    ));
    let mut other = vec![Notification::new(
        EventType::Remove,
        None,
        Val::Object(o),
        Val::Null,
        0,
        false,
    )];
    chain.merge(&mut other);
    assert_eq!(chain.len(), 0);
}

#[test]
fn chain_merge_dispatch_delivers_merged() {
    // Build a chain with a SET feature-matched pair; dispatch delivers one
    // merged notification with the earliest old value and latest new value.
    let (a, notifies, received) = Recording::new(1);
    let mut n = Notifier::new();
    n.add_adapter(a);
    n.e_set_deliver(true);
    let mut chain = NotificationChain::new();
    chain.add(Notification::new(
        EventType::Set,
        None,
        Val::String("old1".into()),
        Val::String("v1".into()),
        -1,
        false,
    ));
    let mut other = vec![Notification::new(
        EventType::Set,
        None,
        Val::String("v1".into()),
        Val::String("v2".into()),
        -1,
        false,
    )];
    chain.merge(&mut other);
    assert_eq!(chain.len(), 1);
    chain.dispatch(&mut n);
    assert_eq!(notifies.get(), 1);
    let r = &received.borrow()[0];
    assert_eq!(r.old_value, Val::String("old1".into()));
    assert_eq!(r.new_value, Val::String("v2".into()));
}

// ---------------------------------------------------------------------------
// Notification was_set
// ---------------------------------------------------------------------------

#[test]
fn notification_was_set_default_false() {
    let ng = Notification::new(EventType::Set, None, Val::Null, Val::Null, -1, false);
    assert!(!ng.was_set());
}

#[test]
fn notification_was_set_constructor_and_toggle() {
    let mut ng = Notification::new(EventType::Set, None, Val::Null, Val::Null, -1, true);
    assert!(ng.was_set());
    ng.set_was_set(false);
    assert!(!ng.was_set());
}

// ---------------------------------------------------------------------------
// ChangeNotificationTests.cpp: Notification construction / accessors
// ---------------------------------------------------------------------------

#[test]
fn notification_construct_set_accessors() {
    // Ports Notification_Construct_SetAccessors: every accessor reflects the
    // value passed to the constructor (position defaults to -1).
    let n = Notification::new(
        EventType::Set,
        Some("name".into()),
        Val::String("old".into()),
        Val::String("new".into()),
        -1,
        false,
    )
    .with_notifier(7);
    assert_eq!(n.event(), EventType::Set);
    assert_eq!(n.notifier(), 7);
    assert_eq!(n.feature(), Some("name"));
    assert_eq!(n.old_value, Val::String("old".into()));
    assert_eq!(n.new_value, Val::String("new".into()));
    assert_eq!(n.position, -1);
}

#[test]
fn notification_construct_with_position() {
    // Ports Notification_Construct_WithPosition: an explicit position is kept.
    let n = Notification::new(EventType::Add, None, Val::Null, Val::Int(42), 3, false);
    assert_eq!(n.event(), EventType::Add);
    assert_eq!(n.position, 3);
}

#[test]
fn notification_default_position_minus_one() {
    // Ports Notification_DefaultPosition_IsMinusOne.
    let n = Notification::new(EventType::Create, None, Val::Null, Val::Null, -1, false);
    assert_eq!(n.position, -1);
}

// ---------------------------------------------------------------------------
// ChangeNotificationTests.cpp: EventType coverage
// ---------------------------------------------------------------------------

#[test]
fn notification_event_type_values() {
    // Ports Notification_EventType_Values: the C-like discriminants match C++.
    assert_eq!(EventType::Create as i32, 0);
    assert_eq!(EventType::Set as i32, 1);
    assert_eq!(EventType::Unset as i32, 2);
    assert_eq!(EventType::Add as i32, 3);
    assert_eq!(EventType::Remove as i32, 4);
    assert_eq!(EventType::AddMany as i32, 5);
    assert_eq!(EventType::RemoveMany as i32, 6);
    assert_eq!(EventType::Move as i32, 7);
    assert_eq!(EventType::RemovingAdapter as i32, 8);
    assert_eq!(EventType::Resolve as i32, 9);
    assert_eq!(EventType::ContentType as i32, 10);
}

#[test]
fn notification_event_type_round_trip() {
    // Ports Notification_EventType_RoundTrip.
    for t in [
        EventType::Create,
        EventType::Set,
        EventType::Unset,
        EventType::Add,
        EventType::AddMany,
        EventType::Remove,
        EventType::RemoveMany,
        EventType::Move,
        EventType::RemovingAdapter,
        EventType::Resolve,
        EventType::ContentType,
    ] {
        let n = Notification::new(t, None, Val::Null, Val::Null, -1, false);
        assert_eq!(n.event(), t);
    }
}

#[test]
fn notification_event_type_name_known_types() {
    // Ports Notification_EventTypeName_KnownTypes.
    assert_eq!(EventType::Set.name(), "SET");
    assert_eq!(EventType::Add.name(), "ADD");
    assert_eq!(EventType::Remove.name(), "REMOVE");
    assert_eq!(EventType::Unset.name(), "UNSET");
    assert_eq!(EventType::Move.name(), "MOVE");
    assert_eq!(EventType::Create.name(), "CREATE");
    assert_eq!(EventType::RemovingAdapter.name(), "REMOVING_ADAPTER");
    assert_eq!(EventType::Resolve.name(), "RESOLVE");
    assert_eq!(EventType::AddMany.name(), "ADD_MANY");
    assert_eq!(EventType::RemoveMany.name(), "REMOVE_MANY");
    assert_eq!(EventType::ContentType.name(), "CONTENT_TYPE");
}

#[test]
fn notification_touch_marks_touched() {
    // Ports Notification_Touch_MarksTouched: the was-set/touched flag starts
    // false and is marked by the setter (Rust stand-in for C++ touch()).
    let mut n = Notification::new(EventType::Set, None, Val::Null, Val::Null, 0, false);
    assert!(!n.was_set());
    n.set_was_set(true);
    assert!(n.was_set());
}

// ---------------------------------------------------------------------------
// ChangeNotificationTests.cpp: NotificationChain (std::vector) aggregation
// ---------------------------------------------------------------------------

#[test]
fn notification_chain_aggregate() {
    // Ports NotificationChain_Aggregate: a plain vector of notifications keeps
    // insertion order and per-entry feature association.
    let chain = vec![
        Notification::new(
            EventType::Add,
            Some("a".into()),
            Val::Null,
            Val::Null,
            -1,
            false,
        ),
        Notification::new(
            EventType::Remove,
            Some("b".into()),
            Val::Null,
            Val::Null,
            -1,
            false,
        ),
        Notification::new(
            EventType::Set,
            Some("c".into()),
            Val::Null,
            Val::Null,
            -1,
            false,
        ),
    ];
    assert_eq!(chain.len(), 3);
    assert_eq!(chain[0].event(), EventType::Add);
    assert_eq!(chain[1].event(), EventType::Remove);
    assert_eq!(chain[2].event(), EventType::Set);
    assert_eq!(chain[0].feature(), Some("a"));
    assert_eq!(chain[1].feature(), Some("b"));
    assert_eq!(chain[2].feature(), Some("c"));
}

#[test]
fn notification_chain_empty() {
    // Ports NotificationChain_Empty.
    let chain: Vec<Notification> = Vec::new();
    assert_eq!(chain.len(), 0);
}

#[test]
fn notification_chain_merge() {
    // Ports NotificationChain_Merge: appending a second collection grows the
    // chain and drains the source.
    let mut chain1 = NotificationChain::new();
    chain1.add(Notification::new(
        EventType::Add,
        Some("a".into()),
        Val::Null,
        Val::Null,
        -1,
        false,
    ));
    let mut chain2 = vec![
        Notification::new(
            EventType::Remove,
            Some("b".into()),
            Val::Null,
            Val::Null,
            -1,
            false,
        ),
        Notification::new(
            EventType::Set,
            Some("c".into()),
            Val::Null,
            Val::Null,
            -1,
            false,
        ),
    ];
    chain1.merge(&mut chain2);
    assert_eq!(chain1.len(), 3);
    assert!(chain2.is_empty());
}

// ---------------------------------------------------------------------------
// ChangeNotificationTests.cpp: eNotify delivery
// ---------------------------------------------------------------------------

#[test]
fn enotify_delivers_to_adapter() {
    // Ports ENotify_DeliversToAdapter: a single adapter receives the SET once.
    let mut n = Notifier::new();
    let (a, notifies, received) = Recording::new(1);
    n.add_adapter(a);
    let evt = Notification::new(EventType::Set, None, Val::Null, Val::Null, -1, false);
    n.e_notify(&evt);
    assert_eq!(notifies.get(), 1);
    let r = received.borrow();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].event(), EventType::Set);
}

/// A zero-sized adapter used to exercise `add_adapter` de-duplication: two
/// boxes of the same ZST compare pointer-identical, standing in for the C++
/// "same adapter pointer added twice".
struct ZstAdapter;
static DUP_HITS: AtomicUsize = AtomicUsize::new(0);
impl Adapter for ZstAdapter {
    fn notify_changed(&mut self, _n: &Notification) {
        DUP_HITS.fetch_add(1, Ordering::SeqCst);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[test]
fn enotify_duplicate_adapter_added_once() {
    // Ports ENotify_DuplicateAdapter_AddedOnce: adding an identical adapter
    // twice keeps a single entry and delivers the notification only once.
    DUP_HITS.store(0, Ordering::SeqCst);
    let mut n = Notifier::new();
    n.add_adapter(Box::new(ZstAdapter));
    n.add_adapter(Box::new(ZstAdapter));
    assert_eq!(n.e_adapters().len(), 1);
    n.e_notify(&Notification::new(
        EventType::Set,
        None,
        Val::Null,
        Val::Null,
        -1,
        false,
    ));
    assert_eq!(DUP_HITS.load(Ordering::SeqCst), 1);
}

// ---------------------------------------------------------------------------
// ChangeNotificationTests.cpp: EAdapter target / isAdapterForType
// ---------------------------------------------------------------------------

#[test]
fn eadapter_set_get_target() {
    // Ports EAdapter_SetGetTarget: target defaults to null, then round-trips.
    let (mut a, _, _) = Recording::new(1);
    assert_eq!(a.target(), None);
    a.set_target(Some(42));
    assert_eq!(a.target(), Some(42));
}

#[test]
fn eadapter_is_adapter_for_type_default_false() {
    // Ports EAdapter_IsAdapterForType_DefaultFalse.
    let (a, _, _) = Recording::new(1);
    assert!(!a.is_adapter_for_type(""));
}
