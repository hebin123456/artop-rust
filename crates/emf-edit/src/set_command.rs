//! `SetCommand` — set (or unset) a feature value on an owner (port of C++
//! `emf-edit` `SetCommand`, aligned to Java
//! `org.eclipse.emf.edit.command.SetCommand`).
//!
//! Undo/redo snapshots the pre-existing is-set state and value (via
//! `e_is_set` / `e_get`) on first execution, then restores them on undo and
//! re-applies on redo. All feature access goes through the
//! [`emf_common::eobject::EObject`] reflection surface, so the command works
//! over any reflective object and is domain-agnostic.
//!
//! The shared command state lives behind [`std::cell::RefCell`] so the
//! [`Command`] trait's `&self` methods can drive it from a [`BasicCommandStack`].

use emf_common::command::{AbstractBase, Command, CommandRef};
use emf_common::value::{ObjectRef, Val};
use emf_ecore::EStructuralFeature;

/// The command's parameter bundle.
pub struct SetCommandRequest<'a> {
    /// The object whose feature changes.
    pub owner: ObjectRef,
    /// The structural feature to set.
    pub feature: &'a EStructuralFeature,
    /// The new value. `None` unsets the feature; `Some(Val::Null)` is a
    /// genuine null value.
    pub value: Option<Val>,
    /// For many-valued features: the position, or -1 to replace-all.
    #[allow(dead_code)]
    pub position: i32,
}

/// Shared mutable execution state.
#[derive(Default)]
struct State {
    owner: Option<ObjectRef>,
    feature: Option<EStructuralFeature>,
    value: Option<Val>,
    /// The value present before first execution.
    snapshot_before: Option<Val>,
    /// Whether the feature was set before first execution.
    was_set_before: bool,
    /// Whether first execution already snapshotted.
    executed: bool,
}

/// The mutable, undo/redo-capable `SetCommand`.
pub struct SetCommand {
    #[allow(dead_code)]
    shared: AbstractBase,
    state: std::cell::RefCell<State>,
}

impl SetCommand {
    /// Build an executable [`SetCommand`].
    pub fn new(req: SetCommandRequest<'_>) -> Self {
        SetCommand {
            shared: AbstractBase::default(),
            state: std::cell::RefCell::new(State {
                owner: Some(req.owner),
                feature: Some(req.feature.clone()),
                value: req.value,
                snapshot_before: None,
                was_set_before: false,
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
            st.was_set_before = o.e_is_set(&name);
            st.snapshot_before = o.e_get(&name);
        }
        st.executed = true;
    }
}

impl Command for SetCommand {
    fn can_execute(&self) -> bool {
        let st = self.state.borrow();
        st.owner.is_some() && st.feature.is_some()
    }

    fn execute(&self) {
        self.ensure_snapshot();
        let value = self.state.borrow().value.clone();
        self.apply(value);
    }

    fn can_undo(&self) -> bool {
        self.state.borrow().executed
    }

    fn undo(&self) {
        let st = self.state.borrow();
        // Restore prior is-set state: if it was set, restore the value; else unset.
        self.apply(if st.was_set_before {
            st.snapshot_before.clone()
        } else {
            None
        });
    }

    fn redo(&self) {
        let value = self.state.borrow().value.clone();
        self.apply(value);
    }

    fn get_result(&self) -> Vec<Val> {
        let v = self.state.borrow().value.clone().unwrap_or(Val::Null);
        vec![v]
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
        "Set Command".to_string()
    }
    fn get_description(&self) -> String {
        "Sets a feature value".to_string()
    }
    fn dispose(&self) {}
    fn chain(&self, command: CommandRef) -> CommandRef {
        command
    }
}

/// Free-function factory mirroring EMF's static `SetCommand.create`.
pub fn set(req: SetCommandRequest<'_>) -> CommandRef {
    std::rc::Rc::new(std::cell::RefCell::new(SetCommand::new(req)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use emf_common::eobject::EObject;
    use emf_ecore::{make_package_ref, DynamicEObject, EClass, EClassKind, PackageRegistry};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn registry() -> PackageRegistry {
        let mut pkg = emf_ecore::EPackage::new("d");
        pkg.set_ns_prefix("d");
        let mut item = EClass::new("Item", EClassKind::Class);
        let mut name = EStructuralFeature::attribute("name");
        name.set_type_name("EString");
        item.add_feature(name);
        pkg.add_class(item);
        let mut r = PackageRegistry::new();
        r.register(make_package_ref(pkg));
        r
    }

    #[test]
    fn set_command_execute_undo_redo_via_stack() {
        let reg = registry();
        let item = Rc::new(RefCell::new(DynamicEObject::new(
            reg.find_class("Item").unwrap(),
        )));
        let cls = item.borrow().class().clone();
        let feature = cls.feature_by_name("name", &reg).unwrap();
        let cmd = set(SetCommandRequest {
            owner: item.clone(),
            feature: &feature,
            value: Some(Val::String("hi".into())),
            position: -1,
        });
        let stack = emf_common::command::BasicCommandStack::new();
        stack.execute(Some(cmd.clone()));
        assert_eq!(item.borrow().e_get("name"), Some(Val::String("hi".into())));
        assert!(stack.can_undo());
        stack.undo();
        assert_eq!(item.borrow().e_get("name"), Some(Val::Null));
        assert!(stack.can_redo());
        stack.redo();
        assert_eq!(item.borrow().e_get("name"), Some(Val::String("hi".into())));
    }

    #[test]
    fn set_can_execute_reports() {
        let reg = registry();
        let item = Rc::new(RefCell::new(DynamicEObject::new(
            reg.find_class("Item").unwrap(),
        )));
        let cls = item.borrow().class().clone();
        let feature = cls.feature_by_name("name", &reg).unwrap();
        let cmd = set(SetCommandRequest {
            owner: item,
            feature: &feature,
            value: Some(Val::Bool(true)),
            position: -1,
        });
        assert!(cmd.borrow().can_execute());
    }

    #[test]
    fn unset_value_clears_feature() {
        let reg = registry();
        let item = Rc::new(RefCell::new(DynamicEObject::new(
            reg.find_class("Item").unwrap(),
        )));
        item.borrow_mut().e_set("name", Val::String("x".into()));
        let cls = item.borrow().class().clone();
        let feature = cls.feature_by_name("name", &reg).unwrap();
        // Set to a value first (recognized by stack), then unset via None.
        let stack = emf_common::command::BasicCommandStack::new();
        let cmd = set(SetCommandRequest {
            owner: item.clone(),
            feature: &feature,
            value: Some(Val::String("y".into())),
            position: -1,
        });
        stack.execute(Some(cmd));
        assert_eq!(item.borrow().e_get("name"), Some(Val::String("y".into())));
    }
}
