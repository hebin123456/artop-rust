//! XMI/XML serialization (port of C++ `emf-xmi`).
//!
//! Port target: C++ `emf-xmi` module of `hebin123456/artop-cpp`.
//!
//! This is the *generic EMF* XMI/XML layer — a pure common base with no
//! knowledge of any domain metamodel (see the decoupling principle in
//! `docs/PROGRESS.md`). The [`saver`] module is a working instance-document
//! serializer; the remaining `*_handler` / load modules below are placeholders
//! that will be filled by subsequent milestones.

pub mod loader;
pub mod options;
pub mod parser;
pub mod saver;
pub mod xmi_resource;
pub mod xmi_resource_factory;
pub mod xml_escape;
pub mod xml_helper;
pub mod xml_load_impl;

pub use xmi_resource::XMIResource;
pub use xmi_resource_factory::XMIResourceFactory;
pub use xml_helper::{FeatureKind, XMLHelper};
pub use xml_load_impl::{XMLLoad, XMLLoadImpl};

pub mod xml_base_handler {
    //! Port target: C++ source unit for `xml_base_handler`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-xmi::xml_base_handler"
    }
}

pub mod sax_mi_handler {
    //! Port target: C++ source unit for `sax_mi_handler`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-xmi::sax_mi_handler"
    }
}

pub mod sax_xml_handler {
    //! Port target: C++ source unit for `sax_xml_handler`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-xmi::sax_xml_handler"
    }
}

pub mod xmi_handler {
    //! Port target: C++ source unit for `xmi_handler`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-xmi::xmi_handler"
    }
}

pub mod xmi_loader {
    //! Port target: C++ source unit for `xmi_loader`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-xmi::xmi_loader"
    }
}

pub mod xmi_saver {
    //! Port target: C++ source unit for `xmi_saver`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-xmi::xmi_saver"
    }
}

pub mod xml_handler {
    //! Port target: C++ source unit for `xml_handler`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-xmi::xml_handler"
    }
}

pub mod xml_save_impl {
    //! Port target: C++ source unit for `xml_save_impl`.
    /// Placeholder marker so the module compiles until the real port lands.
    pub fn api_surface() -> &'static str {
        "emf-xmi::xml_save_impl"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn skeleton_compiles() {
        assert_eq!(
            super::xml_base_handler::api_surface(),
            "emf-xmi::xml_base_handler"
        );
    }
}
