//! Editing / Command framework (port of C++ `emf-edit`).
//!
//! Port target: C++ `emf-edit` module of `hebin123456/artop-cpp`.
//!
//! The editing-domain and the standard commands (`set` / `add` / `remove` /
//! `replace` / `move`) are implemented over the EMF reflection surface, so they
//! operate on any reflective `EObject` and remain artop-agnostic. The
//! transactional editing domain adds notification deferral on top, reusing the
//! `emf-common` deferral queue.

/// Re-exported command handle from `emf-common`.
pub use emf_common::command::CommandRef;

pub mod adapter_factory_editing_domain;

pub mod add_command;

pub mod change_description;

pub mod command_helper;

pub mod composed_adapter_factory;

pub mod edit_plugin;

pub mod edit_util;

pub mod editing_domain;

pub mod move_command;

pub mod provider;

pub mod remove_command;

pub mod replace_command;

pub mod set_command;

pub mod transactional_editing_domain;

pub mod tree_node;

/// Alias so the editing domain's `create_command` can build a Set command via a
/// plain value bundle. Re-exported from `set_command`.
pub use set_command::SetCommandRequest as CommandRequest;

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() {
        assert_eq!(
            super::edit_plugin::EMFEditPlugin::instance().get_string("x"),
            "x"
        );
    }
}
