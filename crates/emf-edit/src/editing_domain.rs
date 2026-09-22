//! `EditingDomain` — the model-editing nucleus (port of C++ `emf-edit`
//! `EditingDomain`, aligned to Java `org.eclipse.emf.edit.domain.EditingDomain`).
//!
//! An editing domain owns the model's command stack and offers a
//! `create_command` shortcut that builds and runs the standard edit commands
//! (`set` / `add` / `remove` / `move`). Rust has no inheritance-based Notifier,
//! so `create_command` is a plain associated function and the domain is a small
//! value holding the [`BasicCommandStack`].
//!
//! Generic EMF only; no domain-metamodel knowledge (see `docs/PROGRESS.md`).

use emf_common::command::BasicCommandStack;
use emf_common::value::Val;
use emf_ecore::EStructuralFeature;

use super::add_command::{add, AddCommandRequest};
use super::move_command::{move_value, MoveCommandRequest};
use super::remove_command::{remove, RemoveCommandRequest};
use super::set_command::set;

/// The kind of edit offered by [`EditingDomain::create_command`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// Set (or unset) a feature value.
    Set,
    /// Add a value to a many-valued feature.
    Add,
    /// Remove a value from a many-valued feature.
    Remove,
    /// Move a value within a many-valued feature.
    Move,
}

/// The parameters [`EditingDomain::create_command`] needs.
pub struct CreateParams {
    /// What edit to build.
    pub kind: CommandKind,
    /// The target object.
    pub owner: emf_common::value::ObjectRef,
    /// The structural feature being edited.
    pub feature: EStructuralFeature,
    /// The value for set/add.
    pub value: Option<Val>,
}

/// A model-editing nucleus: the command stack plus convenience factories.
#[derive(Default)]
pub struct EditingDomain {
    command_stack: BasicCommandStack,
}

impl EditingDomain {
    /// A new domain with an empty command stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// The domain's command stack (EMF `getCommandStack`).
    pub fn command_stack(&self) -> &BasicCommandStack {
        &self.command_stack
    }

    /// Build a command for `params` (without executing it). This mirrors the
    /// C++ `createCommand` entry and keeps the domain as the sole orchestrator.
    pub fn create_command(&self, params: CreateParams) -> Result<crate::CommandRef, String> {
        let owner = params.owner;
        let feature = params.feature;
        match params.kind {
            CommandKind::Set => Ok(set(crate::CommandRequest {
                owner,
                feature: &feature,
                value: params.value,
                position: -1,
            })),
            CommandKind::Add => {
                let value = params
                    .value
                    .ok_or_else(|| "add requires a value".to_string())?;
                Ok(add(AddCommandRequest {
                    owner,
                    feature: feature.clone(),
                    value,
                }))
            }
            CommandKind::Remove => {
                let value = params
                    .value
                    .ok_or_else(|| "remove requires a value".to_string())?;
                Ok(remove(RemoveCommandRequest {
                    owner,
                    feature: feature.clone(),
                    value,
                }))
            }
            CommandKind::Move => {
                let value = params
                    .value
                    .clone()
                    .ok_or_else(|| "move requires a value".to_string())?;
                let new_index = match value {
                    Val::Int(i) => i,
                    _ => {
                        return Err(format!(
                            "move target index must be an integer, got {value:?}"
                        ))
                    }
                };
                let new_index_i32 = i32::try_from(new_index)
                    .map_err(|_| format!("move target index {new_index} out of range"))?;
                Ok(move_value(MoveCommandRequest {
                    owner,
                    feature: feature.clone(),
                    value: params.value.unwrap_or(Val::Null),
                    new_index: new_index_i32,
                }))
            }
        }
    }

    /// Build a Set command with a missing value sentinel (EMF `UNSET`).
    pub fn create_commands_for_set(
        &self,
        owner: emf_common::value::ObjectRef,
        feature: &EStructuralFeature,
        value: Option<Val>,
    ) -> Result<crate::CommandRef, String> {
        Ok(set(crate::CommandRequest {
            owner,
            feature,
            value,
            position: -1,
        }))
    }
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
        let tags =
            EStructuralFeature::new("tags", emf_ecore::structural::FeatureKind::Attribute, 0, -1);
        item.add_feature(name);
        item.add_feature(tags);
        pkg.add_class(item);
        let mut r = PackageRegistry::new();
        r.register(make_package_ref(pkg));
        r
    }

    fn item(reg: &PackageRegistry) -> emf_common::value::ObjectRef {
        let cls = reg.find_class("Item").unwrap();
        Rc::new(RefCell::new(DynamicEObject::new_in(cls, reg.clone())))
    }

    #[test]
    fn create_and_execute_set_via_domain() {
        let reg = registry();
        let item = item(&reg);
        let cls = reg.find_class("Item").unwrap();
        let feature = cls.feature_by_name("name", &reg).unwrap();
        let domain = EditingDomain::new();
        let cmd = domain
            .create_commands_for_set(item.clone(), &feature, Some(Val::String("x".into())))
            .unwrap();
        domain.command_stack().execute(Some(cmd));
        assert_eq!(item.borrow().e_get("name"), Some(Val::String("x".into())));
        // Undo restores the unset default.
        domain.command_stack().undo();
        assert_eq!(item.borrow().e_get("name"), Some(Val::Null));
    }
}
