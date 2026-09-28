//! `CommandHelper` — convenience factories for the standard edit commands
//! (port of C++ `emf-edit` `command/CommandHelper`, aligned to Java
//! `org.eclipse.emf.edit.command.CommandHelper`).
//!
//! Each helper builds one of the standard commands without executing it. The
//! C++ functions also take the owning `EditingDomain`; here the domain is not
//! needed to construct the command (the commands drive the model directly
//! through the reflection surface), so it is omitted.
//!
//! Generic EMF only; no domain-metamodel knowledge (see `docs/PROGRESS.md`).

use emf_common::command::CommandRef;
use emf_common::value::{ObjectRef, Val};
use emf_ecore::EStructuralFeature;

use super::add_command::{add, AddCommandRequest};
use super::move_command::{move_value, MoveCommandRequest};
use super::remove_command::{remove, RemoveCommandRequest};
use super::replace_command::{replace, ReplaceCommandRequest};
use super::set_command::{set, SetCommandRequest};

/// Build an `AddCommand` (C++ `CommandHelper::createAddCommand`).
pub fn create_add_command(owner: ObjectRef, feature: EStructuralFeature, value: Val) -> CommandRef {
    add(AddCommandRequest {
        owner,
        feature,
        value,
    })
}

/// Build a `RemoveCommand` (C++ `CommandHelper::createRemoveCommand`).
pub fn create_remove_command(
    owner: ObjectRef,
    feature: EStructuralFeature,
    value: Val,
) -> CommandRef {
    remove(RemoveCommandRequest {
        owner,
        feature,
        value,
    })
}

/// Build a `SetCommand` (C++ `CommandHelper::createSetCommand`). `None` unsets
/// the feature, matching `SetCommand::UNSET_VALUE`.
pub fn create_set_command(
    owner: ObjectRef,
    feature: &EStructuralFeature,
    value: Option<Val>,
) -> CommandRef {
    set(SetCommandRequest {
        owner,
        feature,
        value,
        position: -1,
    })
}

/// Build a `ReplaceCommand` (C++ `CommandHelper::createReplaceCommand`).
pub fn create_replace_command(
    owner: ObjectRef,
    feature: EStructuralFeature,
    value: Val,
    replacement: Val,
) -> CommandRef {
    replace(ReplaceCommandRequest {
        owner,
        feature,
        value,
        replacement,
    })
}

/// Build a `MoveCommand` (C++ `CommandHelper::createMoveCommand`).
pub fn create_move_command(
    owner: ObjectRef,
    feature: EStructuralFeature,
    value: Val,
    index: i32,
) -> CommandRef {
    move_value(MoveCommandRequest {
        owner,
        feature,
        value,
        new_index: index,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use emf_ecore::{make_package_ref, DynamicEObject, EClass, EClassKind, PackageRegistry};

    use super::*;

    fn model() -> (PackageRegistry, ObjectRef) {
        let mut pkg = emf_ecore::EPackage::new("d");
        pkg.set_ns_prefix("d");
        let mut item = EClass::new("Item", EClassKind::Class);
        let mut name = EStructuralFeature::attribute("name");
        name.set_type_name("EString");
        item.add_feature(name);
        pkg.add_class(item);
        let mut reg = PackageRegistry::new();
        reg.register(make_package_ref(pkg));
        let obj = Rc::new(RefCell::new(DynamicEObject::new_in(
            reg.find_class("Item").unwrap(),
            reg.clone(),
        )));
        (reg, obj)
    }

    #[test]
    fn create_set_command_writes_value() {
        let (reg, obj) = model();
        let feature = reg
            .find_class("Item")
            .unwrap()
            .feature_by_name("name", &reg)
            .unwrap();
        let cmd = create_set_command(obj.clone(), &feature, Some(Val::string("x")));
        cmd.borrow().execute();
        assert_eq!(obj.borrow().e_get("name"), Some(Val::string("x")));
    }
}
