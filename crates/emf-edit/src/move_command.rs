//! `MoveCommand` — move an existing value to a new index within a many-valued
//! feature (port of C++ `emf-edit` `MoveCommand`, aligned to Java
//! `org.eclipse.emf.edit.command.MoveCommand`).
//!
//! Undo/redo: on first execution it records the value's original index, moves
//! it to the target index, and restores the exact prior ordering on undo. All
//! feature access goes through the [`emf_common::eobject::EObject`] reflection
//! surface.

use emf_common::command::{AbstractBase, Command, CommandRef};
use emf_common::value::{ObjectRef, Val};
use emf_ecore::EStructuralFeature;

/// The command's parameter bundle.
pub struct MoveCommandRequest {
    /// The owner whose many-valued feature is reordered.
    pub owner: ObjectRef,
    /// The (many-valued) structural feature.
    pub feature: EStructuralFeature,
    /// The value to move.
    pub value: Val,
    /// The target index after the move.
    pub new_index: i32,
}

/// Shared mutable execution state.
#[derive(Default)]
struct State {
    owner: Option<ObjectRef>,
    feature: Option<EStructuralFeature>,
    value: Option<Val>,
    new_index: i32,
    /// The list value present before first execution.
    snapshot_before: Option<Val>,
    /// The value's original index (recorded on first execution).
    old_index: i32,
    /// Whether first execution already snapshotted.
    executed: bool,
}

/// The mutable, undo/redo-capable `MoveCommand`.
pub struct MoveCommand {
    #[allow(dead_code)]
    shared: AbstractBase,
    state: std::cell::RefCell<State>,
}

impl MoveCommand {
    /// Build an executable [`MoveCommand`].
    pub fn new(req: MoveCommandRequest) -> Self {
        MoveCommand {
            shared: AbstractBase::default(),
            state: std::cell::RefCell::new(State {
                owner: Some(req.owner),
                feature: Some(req.feature),
                value: Some(req.value),
                new_index: req.new_index,
                snapshot_before: None,
                old_index: -1,
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

    /// Move `value` within the current list to `target`. Since Value objects
    /// compare by identity and atoms by value, we locate by equality.
    fn move_in_list(&self, target: i32) -> Option<Val> {
        let st = self.state.borrow();
        let mut list: Vec<Val> = match &st.snapshot_before {
            Some(Val::List(l)) => l.clone(),
            _ => return None,
        };
        let value = st.value.as_ref()?;
        let from = list.iter().position(|v| v == value)?;
        let mut clamped = target.max(0) as usize;
        if clamped >= list.len() {
            clamped = list.len().saturating_sub(1);
        }
        let item = list.remove(from);
        if from < clamped {
            list.insert(clamped, item);
        } else {
            list.insert(clamped, item);
        }
        Some(Val::List(list))
    }

    /// Snapshot the pre-state and record the value's original index.
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
        let (snapshot, old_index) = {
            let mut snap = None;
            let mut old = -1;
            if let Some(owner) = &owner {
                let o = owner.borrow();
                snap = o.e_get(&name);
                if let (Some(Val::List(l)), Some(v)) = (&snap, &st.value) {
                    old = l
                        .iter()
                        .position(|x| x == v)
                        .map(|i| i as i32)
                        .unwrap_or(-1);
                }
            }
            (snap, old)
        };
        st.snapshot_before = snapshot;
        st.old_index = old_index;
        st.executed = true;
    }
}

impl Command for MoveCommand {
    fn can_execute(&self) -> bool {
        let st = self.state.borrow();
        st.owner.is_some() && st.feature.is_some() && st.value.is_some()
    }

    fn execute(&self) {
        self.ensure_snapshot();
        self.apply(self.move_in_list(self.state.borrow().new_index));
    }

    fn can_undo(&self) -> bool {
        self.state.borrow().executed
    }

    fn undo(&self) {
        let state_guard = self.state.borrow();
        let old = state_guard.old_index;
        // Undo moves the value back to its original index.
        self.apply(self.move_in_list(old));
    }

    fn redo(&self) {
        self.apply(self.move_in_list(self.state.borrow().new_index));
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
        "Move Command".to_string()
    }
    fn get_description(&self) -> String {
        "Moves a value within a feature".to_string()
    }
    fn dispose(&self) {}
    fn chain(&self, command: CommandRef) -> CommandRef {
        command
    }
}

/// Free-function factory mirroring EMF's static `MoveCommand.create`.
pub fn move_value(req: MoveCommandRequest) -> CommandRef {
    std::rc::Rc::new(std::cell::RefCell::new(MoveCommand::new(req)))
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
    fn move_execute_undo_redo_via_stack() {
        let reg = registry();
        let item = Rc::new(RefCell::new(DynamicEObject::new(
            reg.find_class("Item").unwrap(),
        )));
        item.borrow_mut().e_set(
            "tags",
            Val::List(vec![
                Val::string("a"),
                Val::string("b"),
                Val::string("c"),
                Val::string("d"),
            ]),
        );
        let cls = item.borrow().class().clone();
        let feature = cls.feature_by_name("tags", &reg).unwrap();
        // Move "c" (index 2) to index 0.
        let cmd = move_value(MoveCommandRequest {
            owner: item.clone(),
            feature,
            value: Val::string("c"),
            new_index: 0,
        });
        let stack = emf_common::command::BasicCommandStack::new();
        stack.execute(Some(cmd.clone()));
        assert_eq!(
            item.borrow().e_get("tags"),
            Some(Val::List(vec![
                Val::string("c"),
                Val::string("a"),
                Val::string("b"),
                Val::string("d")
            ]))
        );
        assert!(stack.can_undo());
        stack.undo();
        assert_eq!(
            item.borrow().e_get("tags"),
            Some(Val::List(vec![
                Val::string("a"),
                Val::string("b"),
                Val::string("c"),
                Val::string("d")
            ]))
        );
        stack.redo();
        assert_eq!(
            item.borrow().e_get("tags"),
            Some(Val::List(vec![
                Val::string("c"),
                Val::string("a"),
                Val::string("b"),
                Val::string("d")
            ]))
        );
    }
}
