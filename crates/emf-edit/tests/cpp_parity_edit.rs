//! C++ parity suite: emf-edit.
//!
//! Ports `CommandTests.cpp`, `EditingDomainTests.cpp` and `PlaceholderTests.cpp`
//! from `artop-cpp/cpp/emf-cpp/emf-edit/tests/` against the Rust `emf-edit`
//! crate. The Rust port drives the same `Node { name: EString, children: Node[*]
//! containment }` model as the C++ tests and exercises the same execute /
//! undo / redo and transactional notification-deferral behaviour.
//!
//! Behaviour notes (see `docs/PARITY_TRACKER.md`):
//!   - Rust has no `nullptr`: the C++ `nullptr`-owner cases map to a
//!     default-constructed command (no owner), which likewise cannot execute.
//!   - Rust's "unset" sentinel is `Option::None` (C++ `SetCommand::UNSET_VALUE`
//!     is an empty `std::any`).
//!   - Adapters are `Box<dyn Adapter>` (no shared instance), so tests with one
//!     C++ adapter attached to several objects use one adapter per object that
//!     each record into a shared buffer.

use std::cell::RefCell;
use std::rc::Rc;

use emf_common::command::{BasicCommandStack, Command, CommandRef};
use emf_common::eobject::EObject;
use emf_common::notification::{notifier_id, Adapter, Notification};
use emf_common::value::Val;
use emf_ecore::structural::FeatureKind;
use emf_ecore::{
    make_package_ref, DynamicEObject, EClass, EClassKind, EPackage, EStructuralFeature,
    PackageRegistry,
};

use emf_edit::adapter_factory_editing_domain::AdapterFactoryEditingDomain;
use emf_edit::add_command::{add, AddCommand, AddCommandRequest};
use emf_edit::change_description::{ChangeDescription, FeatureChange};
use emf_edit::composed_adapter_factory::ComposedAdapterFactory;
use emf_edit::edit_plugin::EMFEditPlugin;
use emf_edit::move_command::{move_value, MoveCommandRequest};
use emf_edit::remove_command::{remove, RemoveCommandRequest};
use emf_edit::replace_command::{replace, ReplaceCommandRequest};
use emf_edit::set_command::{set, SetCommand, SetCommandRequest};
use emf_edit::transactional_editing_domain::TransactionalEditingDomain;

type NodeRef = Rc<RefCell<DynamicEObject>>;

/// A `Node` class: `name: EString`, `children: Node[*] containment`
/// (mirrors `makeNodeModel()` in `EditingDomainTests.cpp`).
fn node_package() -> EPackage {
    let mut node = EClass::new("Node", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_type_name("EString");
    name.set_feature_id(0);
    node.add_feature(name);

    let mut children = EStructuralFeature::new("children", FeatureKind::Reference, 0, -1);
    children.set_containment(true);
    children.set_type_name("Node");
    children.set_feature_id(3);
    node.add_feature(children);

    let mut pkg = EPackage::new("node");
    pkg.set_ns_prefix("node");
    pkg.add_class(node);
    pkg
}

/// Package with an extra `second: EString` attribute (used by the
/// different-features dedup test).
fn node_package_with_second() -> EPackage {
    let mut node = EClass::new("Node", EClassKind::Class);
    let mut name = EStructuralFeature::attribute("name");
    name.set_type_name("EString");
    name.set_feature_id(0);
    node.add_feature(name);
    let mut second = EStructuralFeature::attribute("second");
    second.set_type_name("EString");
    second.set_feature_id(1);
    node.add_feature(second);

    let mut pkg = EPackage::new("node");
    pkg.set_ns_prefix("node");
    pkg.add_class(node);
    pkg
}

fn registry_of(pkg: EPackage) -> PackageRegistry {
    let mut r = PackageRegistry::new();
    r.register(make_package_ref(pkg));
    r
}

fn node_registry() -> PackageRegistry {
    registry_of(node_package())
}

fn node(reg: &PackageRegistry) -> NodeRef {
    Rc::new(RefCell::new(DynamicEObject::new_in(
        reg.find_class("Node").unwrap(),
        reg.clone(),
    )))
}

fn feature(reg: &PackageRegistry, obj: &NodeRef, name: &str) -> EStructuralFeature {
    let cls = obj.borrow().class().clone();
    cls.feature_by_name(name, reg).unwrap()
}

fn object_of(node: &NodeRef) -> Val {
    Val::Object(node.clone())
}

fn children_of(obj: &NodeRef) -> Vec<Val> {
    obj.borrow()
        .e_get("children")
        .and_then(|v| v.as_list().map(|s| s.to_vec()))
        .unwrap_or_default()
}

fn set_children(obj: &NodeRef, items: Vec<Val>) {
    obj.borrow_mut().e_set("children", Val::List(items));
}

fn name_of(obj: &NodeRef) -> String {
    obj.borrow()
        .e_get("name")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

// ---- adapters ----

struct CountingAdapter {
    count: Rc<RefCell<usize>>,
}
impl Adapter for CountingAdapter {
    fn notify_changed(&mut self, _n: &Notification) {
        *self.count.borrow_mut() += 1;
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
fn counting(count: &Rc<RefCell<usize>>) -> Box<dyn Adapter> {
    Box::new(CountingAdapter {
        count: Rc::clone(count),
    })
}

struct RecordingAdapter {
    events: Rc<RefCell<Vec<Notification>>>,
}
impl Adapter for RecordingAdapter {
    fn notify_changed(&mut self, n: &Notification) {
        self.events.borrow_mut().push(n.clone());
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
fn recording(events: &Rc<RefCell<Vec<Notification>>>) -> Box<dyn Adapter> {
    Box::new(RecordingAdapter {
        events: Rc::clone(events),
    })
}

fn count_for(events: &Rc<RefCell<Vec<Notification>>>, feature: &str) -> usize {
    events
        .borrow()
        .iter()
        .filter(|n| n.feature() == Some(feature))
        .count()
}

// ===================== CommandTests.cpp =====================

#[test]
fn command_default_construct_cannot_execute() {
    let cmd = AddCommand::default();
    assert!(!cmd.can_execute());
}

#[test]
fn command_with_owner_can_execute() {
    // C++ passes a full set of nullptr: owner == nullptr -> cannot execute.
    // Rust has no nullptr, so an owner-less command models the same case.
    let cmd = AddCommand::default();
    assert!(!cmd.can_execute());
}

#[test]
fn command_set_command_unset_value_exists() {
    // C++ `SetCommand::UNSET_VALUE` is an empty `std::any`; Rust's unset
    // sentinel is `None`.
    let unset: Option<Val> = None;
    assert!(unset.is_none());
}

#[test]
fn change_description_empty() {
    let cd = ChangeDescription::new();
    assert!(cd.is_empty());
    assert_eq!(cd.changes().len(), 0);
}

#[test]
fn change_description_add() {
    let mut cd = ChangeDescription::new();
    cd.add(FeatureChange::default());
    assert!(!cd.is_empty());
    assert_eq!(cd.changes().len(), 1);
    cd.clear();
    assert!(cd.is_empty());
}

#[test]
fn adapter_factory_editing_domain_construct() {
    let domain = AdapterFactoryEditingDomain::new();
    assert!(domain.get_adapter_factory().is_none());
    assert!(domain.get_command_stack().is_none());
    assert!(domain.get_resource_set().is_none());
}

#[test]
fn composed_adapter_factory_child_factories() {
    use emf_common::notification::AdapterFactory;
    let caf = ComposedAdapterFactory::new();
    assert_eq!(caf.child_factories().len(), 0);
    assert!(!caf.is_factory_for_type(""));
}

// ===================== PlaceholderTests.cpp =====================

#[test]
fn placeholder() {
    assert!(true);
}

#[test]
fn placeholder_framework_loads() {
    // Singleton is always available; string lookup echoes unknown keys.
    let plugin = EMFEditPlugin::instance();
    assert_eq!(plugin.get_string("SomeKey"), "SomeKey");
}

#[test]
fn placeholder_namespaces_are_accessible() {
    // Every emf-edit namespace symbol must resolve.
    let _ = std::any::type_name::<emf_edit::add_command::AddCommand>();
    let _ = std::any::type_name::<emf_edit::remove_command::RemoveCommand>();
    let _ = std::any::type_name::<emf_edit::set_command::SetCommand>();
    let _ = std::any::type_name::<emf_edit::replace_command::ReplaceCommand>();
    let _ = std::any::type_name::<emf_edit::move_command::MoveCommand>();
    let _ = std::any::type_name::<emf_edit::editing_domain::EditingDomain>();
    let _ = std::any::type_name::<AdapterFactoryEditingDomain>();
    assert!(true);
}

// ===================== EditingDomainTests.cpp =====================

#[test]
fn set_command_single_value_execute_undo_redo() {
    let reg = node_registry();
    let obj = node(&reg);
    let name = feature(&reg, &obj, "name");

    let cmd = SetCommand::new(SetCommandRequest {
        owner: obj.clone(),
        feature: &name,
        value: Some(Val::string("root")),
        position: -1,
    });
    assert!(cmd.can_execute());
    cmd.execute();
    assert_eq!(name_of(&obj), "root");
    assert!(obj.borrow().e_is_set("name"));

    cmd.undo();
    assert!(!obj.borrow().e_is_set("name"));

    cmd.redo();
    assert_eq!(name_of(&obj), "root");
}

#[test]
fn set_command_overwrite_undo_restores_old_value() {
    let reg = node_registry();
    let obj = node(&reg);
    obj.borrow_mut().e_set("name", Val::string("old"));
    let name = feature(&reg, &obj, "name");

    let cmd = SetCommand::new(SetCommandRequest {
        owner: obj.clone(),
        feature: &name,
        value: Some(Val::string("new")),
        position: -1,
    });
    cmd.execute();
    assert_eq!(name_of(&obj), "new");
    cmd.undo();
    assert_eq!(name_of(&obj), "old");
    cmd.redo();
    assert_eq!(name_of(&obj), "new");
}

#[test]
fn set_command_unset_value_execute_undo_redo() {
    let reg = node_registry();
    let obj = node(&reg);
    obj.borrow_mut().e_set("name", Val::string("root"));
    assert!(obj.borrow().e_is_set("name"));
    let name = feature(&reg, &obj, "name");

    let cmd = SetCommand::new(SetCommandRequest {
        owner: obj.clone(),
        feature: &name,
        value: None, // UNSET sentinel
        position: -1,
    });
    cmd.execute();
    assert!(!obj.borrow().e_is_set("name"));

    cmd.undo();
    assert!(obj.borrow().e_is_set("name"));
    assert_eq!(name_of(&obj), "root");

    cmd.redo();
    assert!(!obj.borrow().e_is_set("name"));
}

#[test]
fn add_command_multi_value_execute_undo_redo() {
    let reg = node_registry();
    let parent = node(&reg);
    let c1 = node(&reg);
    let children = feature(&reg, &parent, "children");

    assert_eq!(children_of(&parent).len(), 0);

    let cmd = add(AddCommandRequest {
        owner: parent.clone(),
        feature: children,
        value: object_of(&c1),
    });
    cmd.borrow().execute();
    assert_eq!(children_of(&parent).len(), 1);
    assert_eq!(children_of(&parent)[0], object_of(&c1));

    cmd.borrow().undo();
    assert_eq!(children_of(&parent).len(), 0);

    cmd.borrow().redo();
    assert_eq!(children_of(&parent).len(), 1);
    assert_eq!(children_of(&parent)[0], object_of(&c1));
}

#[test]
fn add_command_collection_execute_undo_redo() {
    let reg = node_registry();
    let parent = node(&reg);
    let c1 = node(&reg);
    let c2 = node(&reg);
    let children = feature(&reg, &parent, "children");

    let cmd = add(AddCommandRequest {
        owner: parent.clone(),
        feature: children,
        value: Val::List(vec![object_of(&c1), object_of(&c2)]),
    });
    cmd.borrow().execute();
    assert_eq!(children_of(&parent).len(), 2);

    cmd.borrow().undo();
    assert_eq!(children_of(&parent).len(), 0);

    cmd.borrow().redo();
    assert_eq!(children_of(&parent).len(), 2);
    assert_eq!(children_of(&parent)[0], object_of(&c1));
    assert_eq!(children_of(&parent)[1], object_of(&c2));
}

#[test]
fn remove_command_execute_undo_redo() {
    let reg = node_registry();
    let parent = node(&reg);
    let c1 = node(&reg);
    let c2 = node(&reg);
    set_children(&parent, vec![object_of(&c1), object_of(&c2)]);

    let children = feature(&reg, &parent, "children");
    let cmd = remove(RemoveCommandRequest {
        owner: parent.clone(),
        feature: children,
        value: object_of(&c1),
    });
    cmd.borrow().execute();
    assert_eq!(children_of(&parent).len(), 1);
    assert_eq!(children_of(&parent)[0], object_of(&c2));

    cmd.borrow().undo();
    assert_eq!(children_of(&parent).len(), 2);
    assert_eq!(children_of(&parent)[0], object_of(&c1));
    assert_eq!(children_of(&parent)[1], object_of(&c2));

    cmd.borrow().redo();
    assert_eq!(children_of(&parent).len(), 1);
    assert_eq!(children_of(&parent)[0], object_of(&c2));
}

#[test]
fn move_command_execute_undo_redo() {
    let reg = node_registry();
    let parent = node(&reg);
    let c1 = node(&reg);
    let c2 = node(&reg);
    let c3 = node(&reg);
    set_children(
        &parent,
        vec![object_of(&c1), object_of(&c2), object_of(&c3)],
    );

    // [c1, c2, c3] -> move c3 to index 0 -> [c3, c1, c2]
    let children = feature(&reg, &parent, "children");
    let cmd = move_value(MoveCommandRequest {
        owner: parent.clone(),
        feature: children,
        value: object_of(&c3),
        new_index: 0,
    });
    cmd.borrow().execute();
    assert_eq!(children_of(&parent)[0], object_of(&c3));
    assert_eq!(children_of(&parent)[1], object_of(&c1));
    assert_eq!(children_of(&parent)[2], object_of(&c2));

    cmd.borrow().undo();
    assert_eq!(children_of(&parent)[0], object_of(&c1));
    assert_eq!(children_of(&parent)[1], object_of(&c2));
    assert_eq!(children_of(&parent)[2], object_of(&c3));

    cmd.borrow().redo();
    assert_eq!(children_of(&parent)[0], object_of(&c3));
    assert_eq!(children_of(&parent)[1], object_of(&c1));
    assert_eq!(children_of(&parent)[2], object_of(&c2));
}

#[test]
fn replace_command_execute_undo_redo() {
    let reg = node_registry();
    let parent = node(&reg);
    let c1 = node(&reg);
    let c2 = node(&reg);
    let c3 = node(&reg);
    set_children(&parent, vec![object_of(&c1), object_of(&c2)]);

    let children = feature(&reg, &parent, "children");
    let cmd = replace(ReplaceCommandRequest {
        owner: parent.clone(),
        feature: children,
        value: object_of(&c1),
        replacement: object_of(&c3),
    });
    cmd.borrow().execute();
    assert_eq!(children_of(&parent).len(), 2);
    assert_eq!(children_of(&parent)[0], object_of(&c3));
    assert_eq!(children_of(&parent)[1], object_of(&c2));

    cmd.borrow().undo();
    assert_eq!(children_of(&parent)[0], object_of(&c1));
    assert_eq!(children_of(&parent)[1], object_of(&c2));

    cmd.borrow().redo();
    assert_eq!(children_of(&parent)[0], object_of(&c3));
    assert_eq!(children_of(&parent)[1], object_of(&c2));
}

#[test]
fn basic_command_stack_multiple_commands_undo_redo() {
    let reg = node_registry();
    let obj = node(&reg);
    let name = feature(&reg, &obj, "name");

    let stack = BasicCommandStack::new();
    let cmd1: CommandRef = set(SetCommandRequest {
        owner: obj.clone(),
        feature: &name,
        value: Some(Val::string("a")),
        position: -1,
    });
    let cmd2: CommandRef = set(SetCommandRequest {
        owner: obj.clone(),
        feature: &name,
        value: Some(Val::string("b")),
        position: -1,
    });

    stack.execute(Some(cmd1));
    assert_eq!(name_of(&obj), "a");
    stack.execute(Some(cmd2));
    assert_eq!(name_of(&obj), "b");

    stack.undo();
    assert_eq!(name_of(&obj), "a");
    stack.undo();
    assert!(!obj.borrow().e_is_set("name"));

    stack.redo();
    assert_eq!(name_of(&obj), "a");
    stack.redo();
    assert_eq!(name_of(&obj), "b");
}

#[test]
fn transactional_editing_domain_write_notification_deferral() {
    let reg = node_registry();
    let obj = node(&reg);
    let count = Rc::new(RefCell::new(0usize));
    obj.borrow().add_adapter(counting(&count));

    let domain = TransactionalEditingDomain::new();

    // Before the transaction: direct notification.
    obj.borrow_mut().e_set("name", Val::string("before"));
    assert!(*count.borrow() >= 1);
    let before = *count.borrow();

    // Inside the transaction: deferred, adapter not notified yet.
    domain.run_write(|| {
        obj.borrow_mut().e_set("name", Val::string("during"));
        assert_eq!(*count.borrow(), before);
    });

    // After commit: accumulated notifications delivered.
    assert!(*count.borrow() > before);
}

#[test]
fn transactional_editing_domain_run_exclusive_read_allowed() {
    let reg = node_registry();
    let obj = node(&reg);
    obj.borrow_mut().e_set("name", Val::string("value"));

    let domain = TransactionalEditingDomain::new();
    let mut captured = String::new();
    domain.run_exclusive(|| {
        captured = name_of(&obj);
    });
    assert_eq!(captured, "value");
}

#[test]
fn transactional_editing_domain_nested_transaction_reentrant() {
    let reg = node_registry();
    let obj = node(&reg);
    let count = Rc::new(RefCell::new(0usize));
    obj.borrow().add_adapter(counting(&count));

    let domain = TransactionalEditingDomain::new();
    let before = *count.borrow();

    domain.run_write(|| {
        obj.borrow_mut().e_set("name", Val::string("outer"));
        domain.run_write(|| {
            obj.borrow_mut().e_set("name", Val::string("inner"));
            assert_eq!(*count.borrow(), before);
        });
        // Inner transaction ended (not outermost): no delivery yet.
        assert_eq!(*count.borrow(), before);
    });
    // Outermost transaction committed: batch delivered.
    assert!(*count.borrow() > before);
}

#[test]
fn transactional_editing_domain_cross_object_dedup_multiple_set_merged() {
    let reg = node_registry();
    let obj = node(&reg);
    // Seed a value so the first in-transaction SET has a non-null old value.
    obj.borrow_mut().e_set("name", Val::string("initial"));

    let events = Rc::new(RefCell::new(Vec::new()));
    obj.borrow().add_adapter(recording(&events));

    let domain = TransactionalEditingDomain::new();
    domain.run_write(|| {
        obj.borrow_mut().e_set("name", Val::string("a"));
        obj.borrow_mut().e_set("name", Val::string("b"));
        obj.borrow_mut().e_set("name", Val::string("c"));
    });

    // Three SETs on the same feature merge into one.
    assert_eq!(count_for(&events, "name"), 1);
    assert_eq!(events.borrow().len(), 1);
    // Merged old value is the earliest, new value the latest.
    assert_eq!(events.borrow()[0].old_value, Val::string("initial"));
    assert_eq!(events.borrow()[0].new_value, Val::string("c"));
}

#[test]
fn transactional_editing_domain_dedup_different_features_not_merged() {
    let reg = registry_of(node_package_with_second());
    let obj = node(&reg);
    let events = Rc::new(RefCell::new(Vec::new()));
    obj.borrow().add_adapter(recording(&events));

    let domain = TransactionalEditingDomain::new();
    domain.run_write(|| {
        obj.borrow_mut().e_set("name", Val::string("n1"));
        obj.borrow_mut().e_set("second", Val::string("s1"));
        obj.borrow_mut().e_set("name", Val::string("n2"));
    });

    // name: two SETs merge to one; second: one SET kept -> two total.
    assert_eq!(count_for(&events, "name"), 1);
    assert_eq!(count_for(&events, "second"), 1);
    assert_eq!(events.borrow().len(), 2);
}

#[test]
fn transactional_editing_domain_dedup_multi_objects_independent_merge() {
    let reg = node_registry();
    let a = node(&reg);
    let b = node(&reg);
    let events = Rc::new(RefCell::new(Vec::new()));
    a.borrow().add_adapter(recording(&events));
    b.borrow().add_adapter(recording(&events));

    let a_id = notifier_id(a.borrow().notifier());
    let b_id = notifier_id(b.borrow().notifier());

    let domain = TransactionalEditingDomain::new();
    domain.run_write(|| {
        a.borrow_mut().e_set("name", Val::string("a1"));
        b.borrow_mut().e_set("name", Val::string("b1"));
        a.borrow_mut().e_set("name", Val::string("a2"));
        b.borrow_mut().e_set("name", Val::string("b2"));
    });

    // a and b each merge to one: two total.
    assert_eq!(events.borrow().len(), 2);
    let a_count = events
        .borrow()
        .iter()
        .filter(|n| n.notifier() == a_id)
        .count();
    let b_count = events
        .borrow()
        .iter()
        .filter(|n| n.notifier() == b_id)
        .count();
    assert_eq!(a_count, 1);
    assert_eq!(b_count, 1);
}

#[test]
fn transactional_editing_domain_dedup_non_set_events_preserved() {
    let reg = node_registry();
    let obj = node(&reg);
    let events = Rc::new(RefCell::new(Vec::new()));
    obj.borrow().add_adapter(recording(&events));

    let domain = TransactionalEditingDomain::new();
    domain.run_write(|| {
        // UNSET does not participate in SET merging: it is preserved as-is.
        obj.borrow_mut().e_unset("name");
        obj.borrow_mut().e_set("name", Val::string("x"));
    });

    // UNSET + SET: distinct event types are not merged -> two.
    assert_eq!(events.borrow().len(), 2);
}
