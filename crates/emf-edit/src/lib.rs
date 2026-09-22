//! Editing / Command framework (port of C++ `emf-edit`).
//!
//! Port target: C++ `emf-edit` module of `hebin123456/artop-cpp`.
//!
//! The editing-domain and the standard commands (`set` / `add` / `remove` /
//! `move`) are implemented over the EMF reflection surface, so they operate on
//! any reflective `EObject` and remain artop-agnostic.

/// Re-exported command handle from `emf-common`.
pub use emf_common::command::CommandRef;

pub mod adapter_factory_editing_domain {
    //! Port target: C++ source unit for `adapter_factory_editing_domain`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-edit::adapter_factory_editing_domain"
    }
}

pub mod add_command;

pub mod command_helper {
    //! Port target: C++ source unit for `command_helper`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-edit::command_helper"
    }
}

pub mod composed_adapter_factory {
    //! Port target: C++ source unit for `composed_adapter_factory`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-edit::composed_adapter_factory"
    }
}

pub mod edit_plugin {
    //! Port target: C++ source unit for `edit_plugin`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-edit::edit_plugin"
    }
}

pub mod edit_util {
    //! Port target: C++ source unit for `edit_util`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-edit::edit_util"
    }
}

pub mod editing_domain;

pub mod move_command;

pub mod remove_command;

pub mod replace_command {
    //! Port target: C++ source unit for `replace_command`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-edit::replace_command"
    }
}

pub mod set_command;

/// Alias so the editing domain's `create_command` can build a Set command via a
/// plain value bundle. Re-exported from `set_command`.
pub use set_command::SetCommandRequest as CommandRequest;

pub mod transactional_editing_domain {
    //! Port target: C++ source unit for `transactional_editing_domain`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-edit::transactional_editing_domain"
    }
}

pub mod tree_node {
    //! Port target: C++ source unit for `tree_node`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-edit::tree_node"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() {
        assert_eq!(
            super::adapter_factory_editing_domain::api_surface(),
            "emf-edit::adapter_factory_editing_domain"
        );
    }
}
