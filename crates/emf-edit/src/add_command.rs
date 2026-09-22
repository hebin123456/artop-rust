//! `AddCommand` — add a value to a many-valued feature (port of C++ `emf-edit`
//! `AddCommand`, aligned to Java `org.eclipse.emf.edit.command.AddCommand`).
//!
//! Undo/redo snapshots the pre-existing list on first execution, appends the
//! new value on execute and restores the prior list on undo. All feature access
//! goes through the [`emf_common::eobject::EObject`] reflection surface, so the
//! command works over any reflective object and is domain-agnostic.

use emf_common::command::{AbstractBase, Command, CommandRef};
use emf_common::value::{ObjectRef, Val};
use emf_ecore::EStructuralFeature;

/// The command's parameter bundle.
pub struct AddCommandRequest {
    /// The owner whose many-valued feature grows.
    pub owner: ObjectRef,
    /// The (many-valued) structural feature.
    pub feature: EStructuralFeature,
    /// The value to append.
    pub value: Val,
}

/// Shared mutable execution state.
#[derive(Default)]
struct State {
    owner: Option<ObjectRef>,
    feature: Option<EStructuralFeature>,
    value: Option<Val>,
    /// The list value present before first execution.
    snapshot_before: Option<Val>,
    /// Whether first execution already snapshotted.
    executed: bool,
}

/// The mutable, undo/redo-capable `AddCommand`.
pub struct AddCommand {
    #[allow(dead_code)]
    shared: AbstractBase,
    state: std::cell::RefCell<State>,
}

impl AddCommand {
    /// Build an executable [`AddCommand`].
    pub fn new(req: AddCommandRequest) -> Self {
        AddCommand {
            shared: AbstractBase::default(),
            state: std::cell::RefCell::new(State {
                owner: Some(req.owner),
                feature: Some(req.feature),
                value: Some(req.value),
                snapshot_before: None,
                executed: false,
            }),
        }
    }

    fn apply(&self, value: Option<Val>) {
        let state = self.state.borrow();
        let (Some(owner), Some(feature)) = (&state.owner, &state.feature) else {
            return;
        };
        let name = feature.name();
        let mut o = owner.borrow_mut();
        match value {
            None => {
                let _ = o.e_unset(name);
            }
            Some(v) => {
                let _ = o.e_set(name, v);
            }
        }
    }

    /// Snapshot the pre-state on first execution.
    fn ensure_snapshot(&self) {
        let mut st = self.state.borrow_mut();
        if st.executed {
            return;
        }
        // Clone owner/feature so no borrow of `st` is held during mutation.
        let (owner, name) = {
            let Some(feature) = &st.feature else {
                st.executed = true;
                return;
            };
            (st.owner.clone(), feature.name().to_string())
        };
        if let Some(owner) = &owner {
            let o = owner.borrow();
            st.snapshot_before = o.e_get(&name);
        }
        st.executed = true;
    }

    /// The post-state list: append `value` to the snapshot list (or start a
    /// fresh list when the feature was unset).
    fn build_added(&self) -> Option<Val> {
        let st = self.state.borrow();
        let value = &st.value;
        let mut list: Vec<Val> = match &st.snapshot_before {
            Some(Val::List(l)) => l.clone(),
            _ => Vec::new(),
        };
        if let Some(v) = value {
            list.push(v.clone());
        }
        Some(Val::List(list))
    }
}

impl Command for AddCommand {
    fn can_execute(&self) -> bool {
        let st = self.state.borrow();
        st.owner.is_some() && st.feature.is_some() && st.value.is_some()
    }

    fn execute(&self) {
        self.ensure_snapshot();
        self.apply(self.build_added());
    }

    fn can_undo(&self) -> bool {
        self.state.borrow().executed
    }

    fn undo(&self) {
        let state = self.state.borrow();
        self.apply(state.snapshot_before.clone());
    }

    fn redo(&self) {
        self.apply(self.build_added());
    }

    fn get_result(&self) -> Vec<Val> {
        self.state
            .borrow()
            .value
            .clone()
            .map(|v| vec![v])
            .unwrap_or_default()
    }

    fn get_affected_objects(&self) -> Vec<Val> {
        self.state
            .borrow()
            .owner
            .clone()
            .map(Val::Object)
            .into_iter()
            .collect()
    }

    fn get_label(&self) -> String {
        "Add Command".to_string()
    }
    fn get_description(&self) -> String {
        "Adds a value to a feature".to_string()
    }
    fn dispose(&self) {}
    fn chain(&self, command: CommandRef) -> CommandRef {
        command
    }
}

/// Free-function factory mirroring EMF's static `AddCommand.create`.
pub fn add(req: AddCommandRequest) -> CommandRef {
    std::rc::Rc::new(std::cell::RefCell::new(AddCommand::new(req)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_common::eobject::EObject;
    use emf_ecore::structural::FeatureKind;
    use emf_ecore::{make_package_ref, DynamicEObject, EClass, EClassKind, PackageRegistry};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn registry() -> PackageRegistry {
        let mut pkg = emf_ecore::EPackage::new("d");
        pkg.set_ns_prefix("d");
        let mut item = EClass::new("Item", EClassKind::Class);
        let mut tags = EStructuralFeature::new("tags", FeatureKind::Attribute, 0, -1);
        tags.set_type_name("EString");
        item.add_feature(tags);
        pkg.add_class(item);
        let mut r = PackageRegistry::new();
        r.register(make_package_ref(pkg));
        r
    }

    #[test]
    fn add_execute_undo_redo_via_stack() {
        let reg = registry();
        let item = Rc::new(RefCell::new(DynamicEObject::new(
            reg.find_class("Item").unwrap(),
        )));
        let cls = item.borrow().class().clone();
        let feature = cls.feature_by_name("tags", &reg).unwrap();
        let cmd = add(AddCommandRequest {
            owner: item.clone(),
            feature,
            value: Val::string("t1"),
        });
        let stack = emf_common::command::BasicCommandStack::new();
        stack.execute(Some(cmd.clone()));
        assert_eq!(
            item.borrow().e_get("tags"),
            Some(Val::List(vec![Val::string("t1")]))
        );
        assert!(stack.can_undo());
        stack.undo();
        assert_eq!(item.borrow().e_get("tags"), Some(Val::Null));
        stack.redo();
        assert_eq!(
            item.borrow().e_get("tags"),
            Some(Val::List(vec![Val::string("t1")]))
        );
    }

    #[test]
    fn add_keeps_existing_items() {
        let reg = registry();
        let item = Rc::new(RefCell::new(DynamicEObject::new(
            reg.find_class("Item").unwrap(),
        )));
        item.borrow_mut()
            .e_set("tags", Val::List(vec![Val::string("a")]));
        let cls = item.borrow().class().clone();
        let feature = cls.feature_by_name("tags", &reg).unwrap();
        let cmd = add(AddCommandRequest {
            owner: item.clone(),
            feature,
            value: Val::string("b"),
        });
        let stack = emf_common::command::BasicCommandStack::new();
        stack.execute(Some(cmd));
        assert_eq!(
            item.borrow().e_get("tags"),
            Some(Val::List(vec![Val::string("a"), Val::string("b")]))
        );
    }
}
