//! `ReplaceCommand` — replace one value with another inside a many-valued
//! feature (port of C++ `emf-edit` `ReplaceCommand`, aligned to Java
//! `org.eclipse.emf.edit.command.ReplaceCommand`).
//!
//! On first execution it locates `value` in the feature's list, records the
//! element found there as the undo value, and writes `replacement` in its
//! place. Undo restores the recorded element; redo writes `replacement` again.
//! All access goes through the [`emf_common::eobject::EObject`] reflection
//! surface, so the command is domain-agnostic.

use emf_common::command::{AbstractBase, Command, CommandRef};
use emf_common::value::{ObjectRef, Val};
use emf_ecore::EStructuralFeature;

/// The command's parameter bundle.
pub struct ReplaceCommandRequest {
    /// The owner whose many-valued feature is edited.
    pub owner: ObjectRef,
    /// The (many-valued) structural feature.
    pub feature: EStructuralFeature,
    /// The value to be replaced.
    pub value: Val,
    /// The value to put in its place.
    pub replacement: Val,
}

/// Shared mutable execution state.
#[derive(Default)]
struct State {
    owner: Option<ObjectRef>,
    feature: Option<EStructuralFeature>,
    value: Option<Val>,
    replacement: Option<Val>,
    /// The index at which the value was found (-1 when absent).
    replaced_index: i32,
    /// The element found at that index (the undo value).
    old_value: Option<Val>,
    /// Whether first execution already recorded the index/old value.
    executed: bool,
}

/// The mutable, undo/redo-capable `ReplaceCommand`.
pub struct ReplaceCommand {
    #[allow(dead_code)]
    shared: AbstractBase,
    state: std::cell::RefCell<State>,
}

impl ReplaceCommand {
    /// Build an executable [`ReplaceCommand`].
    pub fn new(req: ReplaceCommandRequest) -> Self {
        ReplaceCommand {
            shared: AbstractBase::default(),
            state: std::cell::RefCell::new(State {
                owner: Some(req.owner),
                feature: Some(req.feature),
                value: Some(req.value),
                replacement: Some(req.replacement),
                replaced_index: -1,
                old_value: None,
                executed: false,
            }),
        }
    }

    /// Read the current list value of the feature.
    fn current_list(&self) -> Option<Vec<Val>> {
        let (owner, name) = {
            let st = self.state.borrow();
            let owner = st.owner.clone()?;
            let feature = st.feature.as_ref()?;
            (owner, feature.name().to_string())
        };
        let guard = owner.borrow();
        let result = guard
            .e_get(&name)
            .and_then(|v| v.as_list().map(|s| s.to_vec()));
        result
    }

    /// Write `value` at `index` in the feature's list.
    fn set_index(&self, index: i32, value: Val) {
        let (owner, name) = {
            let st = self.state.borrow();
            let (Some(owner), Some(feature)) = (&st.owner, &st.feature) else {
                return;
            };
            (owner.clone(), feature.name().to_string())
        };
        let mut list = match owner
            .borrow()
            .e_get(&name)
            .and_then(|v| v.as_list().map(|s| s.to_vec()))
        {
            Some(l) => l,
            None => return,
        };
        if index < 0 || index as usize >= list.len() {
            return;
        }
        list[index as usize] = value;
        owner.borrow_mut().e_set(&name, Val::List(list));
    }
}

impl Command for ReplaceCommand {
    fn can_execute(&self) -> bool {
        let st = self.state.borrow();
        st.owner.is_some() && st.feature.is_some()
    }

    fn execute(&self) {
        let list = self.current_list();
        let mut st = self.state.borrow_mut();
        let value = st.value.clone();
        if !st.executed {
            st.replaced_index = match (&list, &value) {
                (Some(l), Some(v)) => l
                    .iter()
                    .position(|x| x == v)
                    .map(|i| i as i32)
                    .unwrap_or(-1),
                _ => -1,
            };
            if st.replaced_index >= 0 {
                st.old_value = list
                    .as_ref()
                    .and_then(|l| l.get(st.replaced_index as usize).cloned());
            }
            st.executed = true;
        }
        let index = st.replaced_index;
        let replacement = st.replacement.clone();
        drop(st);
        if let Some(r) = replacement {
            self.set_index(index, r);
        }
    }

    fn can_undo(&self) -> bool {
        let st = self.state.borrow();
        st.executed && st.replaced_index >= 0
    }

    fn undo(&self) {
        let st = self.state.borrow();
        let index = st.replaced_index;
        let old = st.old_value.clone();
        drop(st);
        if let Some(o) = old {
            self.set_index(index, o);
        }
    }

    fn redo(&self) {
        let st = self.state.borrow();
        let index = st.replaced_index;
        let replacement = st.replacement.clone();
        drop(st);
        if let Some(r) = replacement {
            self.set_index(index, r);
        }
    }

    fn get_result(&self) -> Vec<Val> {
        self.state
            .borrow()
            .replacement
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
        "Replace Command".to_string()
    }
    fn get_description(&self) -> String {
        "Replaces a value in a feature".to_string()
    }
    fn dispose(&self) {}
    fn chain(&self, command: CommandRef) -> CommandRef {
        command
    }
}

/// Free-function factory mirroring EMF's static `ReplaceCommand.create`.
pub fn replace(req: ReplaceCommandRequest) -> CommandRef {
    std::rc::Rc::new(std::cell::RefCell::new(ReplaceCommand::new(req)))
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
        let tags = EStructuralFeature::new("tags", FeatureKind::Attribute, 0, -1);
        item.add_feature(tags);
        pkg.add_class(item);
        let mut r = PackageRegistry::new();
        r.register(make_package_ref(pkg));
        r
    }

    #[test]
    fn replace_execute_undo_redo() {
        let reg = registry();
        let item = Rc::new(RefCell::new(DynamicEObject::new(
            reg.find_class("Item").unwrap(),
        )));
        item.borrow_mut()
            .e_set("tags", Val::List(vec![Val::string("a"), Val::string("b")]));
        let cls = item.borrow().class().clone();
        let feature = cls.feature_by_name("tags", &reg).unwrap();
        let cmd = replace(ReplaceCommandRequest {
            owner: item.clone(),
            feature,
            value: Val::string("a"),
            replacement: Val::string("c"),
        });
        let stack = emf_common::command::BasicCommandStack::new();
        stack.execute(Some(cmd));
        assert_eq!(
            item.borrow().e_get("tags"),
            Some(Val::List(vec![Val::string("c"), Val::string("b")]))
        );
        stack.undo();
        assert_eq!(
            item.borrow().e_get("tags"),
            Some(Val::List(vec![Val::string("a"), Val::string("b")]))
        );
        stack.redo();
        assert_eq!(
            item.borrow().e_get("tags"),
            Some(Val::List(vec![Val::string("c"), Val::string("b")]))
        );
    }
}
